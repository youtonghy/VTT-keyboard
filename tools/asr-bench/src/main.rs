//! 本地 ASR 引擎基准测试（sherpa-onnx）。
//!
//! 每次运行只测一个引擎，便于用 ru_maxrss 统计该引擎的峰值内存。
//! 用法见 tools/asr-bench/README.md。

use sherpa_onnx::{
    OfflineMoonshineModelConfig, OfflineRecognizer, OfflineRecognizerConfig,
    OfflineSenseVoiceModelConfig, OfflineTransducerModelConfig, OnlineRecognizer,
    OnlineRecognizerConfig, OnlineTransducerModelConfig,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

/// 流式模型模拟实时输入时每次送入的音频长度
const STREAM_CHUNK_SECONDS: f32 = 0.1;
/// 流式模型在输入结束后补的静音，保证最后一个 chunk 被完整解码
const STREAM_TAIL_PADDING_SECONDS: f32 = 0.6;

#[derive(Clone, Copy)]
enum Kind {
    SenseVoice,
    /// NeMo TDT 离线 transducer（Parakeet）
    NemoTransducer,
    /// Moonshine v2：encoder + merged decoder
    MoonshineV2,
    /// 流式 transducer（Nemotron）
    OnlineTransducer,
}

struct Engine {
    name: &'static str,
    dir: &'static str,
    kind: Kind,
}

const ENGINES: &[Engine] = &[
    Engine {
        name: "sensevoice-int8",
        dir: "sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17",
        kind: Kind::SenseVoice,
    },
    Engine {
        name: "parakeet-tdt-0.6b-v3-int8",
        dir: "sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8",
        kind: Kind::NemoTransducer,
    },
    Engine {
        name: "nemotron-en-0.6b-560ms-int8",
        dir: "sherpa-onnx-nemotron-speech-streaming-en-0.6b-560ms-int8-2026-04-25",
        kind: Kind::OnlineTransducer,
    },
    Engine {
        name: "nemotron-3.5-0.6b-560ms-int8",
        dir: "sherpa-onnx-nemotron-3.5-asr-streaming-0.6b-560ms-int8-2026-06-11",
        kind: Kind::OnlineTransducer,
    },
    Engine {
        name: "moonshine-tiny-en",
        dir: "sherpa-onnx-moonshine-tiny-en-quantized-2026-02-27",
        kind: Kind::MoonshineV2,
    },
    Engine {
        name: "moonshine-base-en",
        dir: "sherpa-onnx-moonshine-base-en-quantized-2026-02-27",
        kind: Kind::MoonshineV2,
    },
    Engine {
        name: "moonshine-base-zh",
        dir: "sherpa-onnx-moonshine-base-zh-quantized-2026-02-27",
        kind: Kind::MoonshineV2,
    },
];

struct Args {
    engine: String,
    models_dir: PathBuf,
    manifest: PathBuf,
    threads: i32,
    language: String,
    show_text: bool,
    /// 在每段音频前补的静音秒数，模拟按下快捷键到开口之间的停顿
    lead_silence: f32,
}

fn usage() -> String {
    let names: Vec<_> = ENGINES.iter().map(|e| e.name).collect();
    format!(
        "usage: asr-bench --engine <name> --models-dir <dir> --manifest <tsv> \
         [--threads N] [--language auto|zh|en|...] [--lead-silence SECS] [--show-text]\nengines: {}",
        names.join(", ")
    )
}

fn parse_args() -> Result<Args, String> {
    let mut engine = None;
    let mut models_dir = None;
    let mut manifest = None;
    let mut threads = 2;
    let mut language = "auto".to_string();
    let mut show_text = false;
    let mut lead_silence = 0.0;
    let mut iter = std::env::args().skip(1);
    while let Some(flag) = iter.next() {
        let mut value = || iter.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--engine" => engine = Some(value()?),
            "--models-dir" => models_dir = Some(PathBuf::from(value()?)),
            "--manifest" => manifest = Some(PathBuf::from(value()?)),
            "--threads" => {
                threads = value()?
                    .parse()
                    .map_err(|err| format!("invalid --threads: {err}"))?
            }
            "--language" => language = value()?,
            "--show-text" => show_text = true,
            "--lead-silence" => {
                lead_silence = value()?
                    .parse()
                    .map_err(|err| format!("invalid --lead-silence: {err}"))?
            }
            "-h" | "--help" => return Err(usage()),
            other => return Err(format!("unknown argument {other}\n{}", usage())),
        }
    }
    Ok(Args {
        engine: engine.ok_or_else(usage)?,
        models_dir: models_dir.ok_or_else(usage)?,
        manifest: manifest.ok_or_else(usage)?,
        threads,
        language,
        show_text,
        lead_silence,
    })
}

/// 清单行：`<wav 路径>\t<分组>\t<参考文本>`；参考文本为空表示只测速度
struct Case {
    wav: PathBuf,
    group: String,
    reference: String,
}

fn load_manifest(path: &Path) -> Result<Vec<Case>, String> {
    let text = fs::read_to_string(path).map_err(|err| format!("read manifest: {err}"))?;
    let base = path.parent().unwrap_or(Path::new("."));
    let mut cases = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.trim_end();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut cols = line.splitn(3, '\t');
        let wav = cols.next().unwrap_or_default();
        let group = cols
            .next()
            .ok_or_else(|| format!("manifest line {}: missing group", index + 1))?;
        let reference = cols.next().unwrap_or_default();
        let wav = PathBuf::from(wav);
        cases.push(Case {
            wav: if wav.is_absolute() {
                wav
            } else {
                base.join(wav)
            },
            group: group.to_string(),
            reference: reference.to_string(),
        });
    }
    Ok(cases)
}

/// 读取 16-bit PCM WAV 并下混为单声道（与应用 sherpa.rs 的处理一致）
fn read_wav(path: &Path) -> Result<(Vec<f32>, i32), String> {
    let mut reader =
        hound::WavReader::open(path).map_err(|err| format!("{}: {err}", path.display()))?;
    let spec = reader.spec();
    if spec.bits_per_sample != 16 || spec.sample_format != hound::SampleFormat::Int {
        return Err(format!(
            "{}: only 16-bit PCM WAV is supported",
            path.display()
        ));
    }
    let channels = usize::from(spec.channels.max(1));
    let samples = reader
        .samples::<i16>()
        .map(|s| s.map(|v| f32::from(v) / f32::from(i16::MAX)))
        .collect::<Result<Vec<f32>, _>>()
        .map_err(|err| format!("{}: {err}", path.display()))?;
    let mono = if channels == 1 {
        samples
    } else {
        samples
            .chunks_exact(channels)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
            .collect()
    };
    Ok((mono, spec.sample_rate as i32))
}

/// 在模型目录中查找 `<prefix>*.onnx` / `<prefix>*.ort`，优先 int8 量化文件
fn find_onnx(dir: &Path, prefix: &str) -> Result<String, String> {
    let mut matches: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|err| format!("{}: {err}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            name.starts_with(prefix) && (name.ends_with(".onnx") || name.ends_with(".ort"))
        })
        .collect();
    matches.sort_by_key(|path| !path.to_string_lossy().contains("int8"));
    matches
        .first()
        .map(|path| path.to_string_lossy().to_string())
        .ok_or_else(|| format!("{}: no {prefix}*.onnx|.ort", dir.display()))
}

fn path_str(path: PathBuf) -> String {
    path.to_string_lossy().to_string()
}

enum Recognizer {
    Offline(OfflineRecognizer),
    Online(OnlineRecognizer),
}

fn create_recognizer(engine: &Engine, dir: &Path, args: &Args) -> Result<Recognizer, String> {
    let tokens = Some(path_str(dir.join("tokens.txt")));
    match engine.kind {
        Kind::OnlineTransducer => {
            let mut config = OnlineRecognizerConfig::default();
            config.model_config.transducer = OnlineTransducerModelConfig {
                encoder: Some(find_onnx(dir, "encoder")?),
                decoder: Some(find_onnx(dir, "decoder")?),
                joiner: Some(find_onnx(dir, "joiner")?),
            };
            config.model_config.tokens = tokens;
            config.model_config.num_threads = args.threads;
            OnlineRecognizer::create(&config)
                .map(Recognizer::Online)
                .ok_or_else(|| "failed to create online recognizer".to_string())
        }
        kind => {
            let mut config = OfflineRecognizerConfig::default();
            match kind {
                Kind::SenseVoice => {
                    config.model_config.sense_voice = OfflineSenseVoiceModelConfig {
                        model: Some(find_onnx(dir, "model")?),
                        language: Some(args.language.clone()),
                        use_itn: true,
                    };
                }
                Kind::NemoTransducer => {
                    config.model_config.transducer = OfflineTransducerModelConfig {
                        encoder: Some(find_onnx(dir, "encoder")?),
                        decoder: Some(find_onnx(dir, "decoder")?),
                        joiner: Some(find_onnx(dir, "joiner")?),
                    };
                    config.model_config.model_type = Some("nemo_transducer".to_string());
                }
                Kind::MoonshineV2 => {
                    config.model_config.moonshine = OfflineMoonshineModelConfig {
                        encoder: Some(find_onnx(dir, "encoder")?),
                        merged_decoder: Some(find_onnx(dir, "decoder")?),
                        ..Default::default()
                    };
                }
                Kind::OnlineTransducer => unreachable!(),
            }
            config.model_config.tokens = tokens;
            config.model_config.num_threads = args.threads;
            OfflineRecognizer::create(&config)
                .map(Recognizer::Offline)
                .ok_or_else(|| "failed to create offline recognizer".to_string())
        }
    }
}

struct Decoded {
    text: String,
    /// 全部推理耗时
    compute_secs: f64,
    /// 松开按键（音频结束）到拿到最终文本的耗时
    release_latency_secs: f64,
}

fn decode(recognizer: &Recognizer, samples: &[f32], rate: i32, language: &str) -> Decoded {
    match recognizer {
        Recognizer::Offline(rec) => {
            let start = Instant::now();
            let stream = rec.create_stream();
            stream.accept_waveform(rate, samples);
            rec.decode(&stream);
            let text = stream.get_result().map(|r| r.text).unwrap_or_default();
            let secs = start.elapsed().as_secs_f64();
            Decoded {
                text,
                compute_secs: secs,
                release_latency_secs: secs,
            }
        }
        Recognizer::Online(rec) => {
            // 按 100ms 分块送入，模拟边说边识别；说话期间的计算不计入松键延迟
            let stream = rec.create_stream();
            if language != "auto" {
                stream.set_option("language", language);
            }
            let chunk = ((rate as f32) * STREAM_CHUNK_SECONDS) as usize;
            let mut compute = 0.0;
            for piece in samples.chunks(chunk.max(1)) {
                let start = Instant::now();
                stream.accept_waveform(rate, piece);
                while rec.is_ready(&stream) {
                    rec.decode(&stream);
                }
                compute += start.elapsed().as_secs_f64();
            }
            let release = Instant::now();
            let padding = vec![0.0_f32; ((rate as f32) * STREAM_TAIL_PADDING_SECONDS) as usize];
            stream.accept_waveform(rate, &padding);
            stream.input_finished();
            while rec.is_ready(&stream) {
                rec.decode(&stream);
            }
            let text = rec.get_result(&stream).map(|r| r.text).unwrap_or_default();
            let release_secs = release.elapsed().as_secs_f64();
            Decoded {
                text,
                compute_secs: compute + release_secs,
                release_latency_secs: release_secs,
            }
        }
    }
}

/// 混合分词：每个汉字/假名/韩文字符算一个 token，连续的字母数字算一个单词。
/// 忽略大小写与标点，中文按 CER、英文按 WER、中英混合按 MER 统一计算。
fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    for ch in text.chars().flat_map(char::to_lowercase) {
        if is_cjk(ch) {
            if !word.is_empty() {
                tokens.push(std::mem::take(&mut word));
            }
            tokens.push(ch.to_string());
        } else if ch.is_alphanumeric() {
            word.push(ch);
        } else if ch == '\'' || ch == '’' {
            // don't / it's 视为一个词
        } else if !word.is_empty() {
            tokens.push(std::mem::take(&mut word));
        }
    }
    if !word.is_empty() {
        tokens.push(word);
    }
    tokens
}

fn is_cjk(ch: char) -> bool {
    matches!(ch as u32,
        0x3040..=0x30FF   // 平假名、片假名
        | 0x3400..=0x4DBF // CJK 扩展 A
        | 0x4E00..=0x9FFF // CJK 统一表意文字
        | 0xAC00..=0xD7AF // 韩文音节
        | 0xF900..=0xFAFF)
}

fn edit_distance(a: &[String], b: &[String]) -> usize {
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, x) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, y) in b.iter().enumerate() {
            let substitution = prev[j] + usize::from(x != y);
            cur[j + 1] = substitution.min(prev[j + 1] + 1).min(cur[j] + 1);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// 进程峰值常驻内存（MB）
fn peak_rss_mb() -> f64 {
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    // SAFETY: getrusage 只写入传入的结构体
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) } != 0 {
        return f64::NAN;
    }
    let raw = usage.ru_maxrss as f64;
    // macOS 以字节为单位，Linux 以 KB 为单位
    if cfg!(target_os = "macos") {
        raw / 1024.0 / 1024.0
    } else {
        raw / 1024.0
    }
}

#[derive(Default)]
struct GroupStats {
    cases: usize,
    edits: usize,
    ref_tokens: usize,
    audio_secs: f64,
    compute_secs: f64,
    release_secs: Vec<f64>,
}

fn percentile(values: &mut [f64], p: f64) -> f64 {
    if values.is_empty() {
        return f64::NAN;
    }
    values.sort_by(|a, b| a.total_cmp(b));
    let index = ((values.len() - 1) as f64 * p).round() as usize;
    values[index]
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let engine = ENGINES
        .iter()
        .find(|e| e.name == args.engine)
        .ok_or_else(|| format!("unknown engine {}\n{}", args.engine, usage()))?;
    let dir = args.models_dir.join(engine.dir);
    if !dir.is_dir() {
        return Err(format!("model directory not found: {}", dir.display()));
    }
    let cases = load_manifest(&args.manifest)?;

    let load_start = Instant::now();
    let recognizer = create_recognizer(engine, &dir, &args)?;
    let load_secs = load_start.elapsed().as_secs_f64();

    // 预热一次，排除首次推理的图优化开销
    let (warm, warm_rate) = read_wav(&cases.first().ok_or("manifest is empty")?.wav)?;
    let _ = decode(&recognizer, &warm, warm_rate, &args.language);

    let mut groups: Vec<(String, GroupStats)> = Vec::new();
    for case in &cases {
        let (mut samples, rate) = read_wav(&case.wav)?;
        if args.lead_silence > 0.0 {
            let lead = (rate as f32 * args.lead_silence) as usize;
            samples.splice(0..0, std::iter::repeat_n(0.0, lead));
        }
        let audio_secs = samples.len() as f64 / rate as f64;
        let out = decode(&recognizer, &samples, rate, &args.language);
        let hyp = tokenize(&out.text);
        let reference = tokenize(&case.reference);
        let edits = if reference.is_empty() {
            0
        } else {
            edit_distance(&reference, &hyp)
        };
        if args.show_text {
            println!(
                "  [{}] {} | {:.2}s | {}",
                case.group,
                case.wav.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
                out.release_latency_secs,
                out.text.trim()
            );
        }
        let index = match groups.iter().position(|(g, _)| *g == case.group) {
            Some(index) => index,
            None => {
                groups.push((case.group.clone(), GroupStats::default()));
                groups.len() - 1
            }
        };
        let stats = &mut groups[index].1;
        stats.cases += 1;
        stats.edits += edits;
        stats.ref_tokens += reference.len();
        stats.audio_secs += audio_secs;
        stats.compute_secs += out.compute_secs;
        stats.release_secs.push(out.release_latency_secs);
    }

    println!(
        "engine={} load={:.2}s peak_rss={:.0}MB threads={}",
        engine.name,
        load_secs,
        peak_rss_mb(),
        args.threads
    );
    for (group, stats) in &mut groups {
        let err = if stats.ref_tokens == 0 {
            "-".to_string()
        } else {
            format!(
                "{:.1}%",
                100.0 * stats.edits as f64 / stats.ref_tokens as f64
            )
        };
        println!(
            "RESULT\t{}\t{}\tn={}\terr={}\trtf={:.3}\trelease_p50={:.3}s\trelease_max={:.3}s",
            engine.name,
            group,
            stats.cases,
            err,
            stats.compute_secs / stats.audio_secs,
            percentile(&mut stats.release_secs, 0.5),
            percentile(&mut stats.release_secs, 1.0),
        );
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}
