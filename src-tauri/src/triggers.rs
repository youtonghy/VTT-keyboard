use crate::openai::{self, OpenAiError};
use crate::settings::{Settings, TriggerCard};
use regex::Regex;

pub struct TriggerResult {
    pub output: String,
    pub triggered: bool,
}

pub fn apply_triggers(
    settings: &Settings,
    input: &str,
    log: &dyn Fn(&str),
) -> Result<TriggerResult, OpenAiError> {
    let sentences = split_sentences(input);
    let mut output = input.to_string();
    let mut triggered = false;

    for card in settings.triggers.iter().filter(|card| card.enabled) {
        if card.variables.is_empty() {
            continue;
        }
        let matched = match_card(card, &sentences)
            .map(|value| (value, true))
            .or_else(|| {
                if card.auto_apply {
                    card
                        .variables
                        .iter()
                        .find(|value| !value.trim().is_empty())
                        .cloned()
                        .map(|value| (value, false))
                } else {
                    None
                }
            });

        if let Some((value, matched_by_keyword)) = matched {
            #[cfg(debug_assertions)]
            {
                log(&format!(
                    "触发卡片 {} (id: {}), value: {}, mode: {}",
                    card.title,
                    card.id,
                    value,
                    if matched_by_keyword { "keyword" } else { "auto" }
                ));
            }
            let cleaned = if matched_by_keyword {
                remove_trigger_phrase(&output, &card.keyword)
            } else {
                output.clone()
            };
            let prompt = card
                .prompt_template
                .replace("{value}", &value)
                .replace("{language}", &value)
                .replace("{style}", &value);
            let instructions = merge_instructions(&settings.openai.text.instructions, &prompt);
            output = openai::generate_text(settings, &cleaned, &instructions)?;
            #[cfg(debug_assertions)]
            {
                log(&format!("触发卡片 {} 结果: {}", card.id, output));
            }
            triggered = true;
        }
    }

    Ok(TriggerResult { output, triggered })
}

fn split_sentences(input: &str) -> Vec<String> {
    input
        .split([',', '，'])
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect()
}

fn match_card(card: &TriggerCard, sentences: &[String]) -> Option<String> {
    let first = sentences.first()?;
    let last = sentences.last()?;
    capture_phrase(card, first).or_else(|| capture_phrase(card, last))
}

fn capture_phrase(card: &TriggerCard, sentence: &str) -> Option<String> {
    if card.keyword.trim().is_empty() {
        return None;
    }
    let pattern = format!(r"(?i){}(?P<value>[^,，。！？!?.]*)", regex::escape(&card.keyword));
    let regex = Regex::new(&pattern).ok()?;
    let value = regex
        .captures(sentence)
        .and_then(|caps| caps.name("value"))
        .map(|value| value.as_str().trim().to_string())?;
    if value.is_empty() {
        return None;
    }
    let allowed = card
        .variables
        .iter()
        .filter(|item| !item.trim().is_empty())
        .any(|item| item.trim().eq_ignore_ascii_case(&value));
    if allowed {
        Some(value)
    } else {
        None
    }
}

fn remove_trigger_phrase(input: &str, keyword: &str) -> String {
    let pattern = format!(r"(?i){}[^,，。！？!?.]*", regex::escape(keyword));
    let regex = Regex::new(&pattern).ok();
    let cleaned = regex
        .map(|re| re.replace_all(input, ""))
        .unwrap_or_else(|| input.into());
    cleaned.trim().to_string()
}

fn merge_instructions(base: &str, extra: &str) -> String {
    if base.trim().is_empty() {
        return extra.to_string();
    }
    if extra.trim().is_empty() {
        return base.to_string();
    }
    format!("{}\n{}", base.trim(), extra.trim())
}
