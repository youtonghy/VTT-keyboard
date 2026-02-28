use crate::openai::{self, OpenAiError};
use crate::settings::{Settings, TriggerCard, TriggerMatch, TriggerMatchMode};
use regex::Regex;

const VALUE_PLACEHOLDER: &str = "{value}";
const SENTENCE_DELIMITERS: [char; 12] =
    [',', '，', '。', '.', '!', '！', '?', '？', ';', '；', ':', '：'];

pub struct TriggerResult {
    pub output: String,
    pub triggered: bool,
    pub triggered_by_keyword: bool,
    pub trigger_matches: Vec<TriggerMatch>,
}

pub fn apply_triggers(
    settings: &Settings,
    input: &str,
    log: &dyn Fn(&str),
) -> Result<TriggerResult, OpenAiError> {
    let sentences = split_sentences(input);
    let mut output = input.to_string();
    let mut triggered = false;
    let mut triggered_by_keyword = false;
    let mut trigger_matches = Vec::new();

    for card in settings.triggers.iter().filter(|card| card.enabled) {
        let matched = match_card(card, &sentences).or_else(|| {
            if card.auto_apply {
                first_non_empty_variable(card).map(|value| (value, false))
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
            if matched_by_keyword {
                triggered_by_keyword = true;
            }
            trigger_matches.push(TriggerMatch {
                trigger_id: card.id.clone(),
                trigger_title: card.title.clone(),
                keyword: card.keyword.clone(),
                matched_value: value,
                mode: if matched_by_keyword {
                    TriggerMatchMode::Keyword
                } else {
                    TriggerMatchMode::Auto
                },
            });
            triggered = true;
        }
    }

    Ok(TriggerResult {
        output,
        triggered,
        triggered_by_keyword,
        trigger_matches,
    })
}

fn split_sentences(input: &str) -> Vec<String> {
    input
        .split(SENTENCE_DELIMITERS)
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect()
}

fn match_card(card: &TriggerCard, sentences: &[String]) -> Option<(String, bool)> {
    let sentence = find_keyword_sentence(card, sentences)?;
    let value = match_variable_in_sentence(sentence, &card.variables)
        .or_else(|| first_non_empty_variable(card))?;
    Some((value, true))
}

fn find_keyword_sentence<'a>(card: &TriggerCard, sentences: &'a [String]) -> Option<&'a str> {
    let keyword = card.keyword.trim();
    if keyword.is_empty() {
        return None;
    }

    if let Some((prefix, suffix)) = split_keyword(keyword) {
        return sentences
            .iter()
            .find(|sentence| match_sentence(sentence, prefix, suffix).is_some())
            .map(String::as_str);
    }

    let normalized_keyword = normalize_for_compare(keyword);
    if normalized_keyword.is_empty() {
        return None;
    }
    sentences
        .iter()
        .find(|sentence| normalize_for_compare(sentence).contains(&normalized_keyword))
        .map(String::as_str)
}

fn match_variable_in_sentence(sentence: &str, variables: &[String]) -> Option<String> {
    let normalized_sentence = normalize_for_compare(sentence);
    if normalized_sentence.is_empty() {
        return None;
    }

    let mut matched: Option<(usize, usize, String)> = None;
    for variable in variables {
        let trimmed = variable.trim();
        if trimmed.is_empty() {
            continue;
        }
        let normalized_variable = normalize_for_compare(trimmed);
        if normalized_variable.is_empty() {
            continue;
        }

        if let Some(start) = normalized_sentence.find(&normalized_variable) {
            let length = normalized_variable.chars().count();
            let should_replace = match matched.as_ref() {
                Some((best_start, best_len, _)) => {
                    start < *best_start || (start == *best_start && length > *best_len)
                }
                None => true,
            };
            if should_replace {
                matched = Some((start, length, trimmed.to_string()));
            }
        }
    }

    matched.map(|(_, _, value)| value)
}

fn first_non_empty_variable(card: &TriggerCard) -> Option<String> {
    card.variables
        .iter()
        .find_map(|value| (!value.trim().is_empty()).then(|| value.trim().to_string()))
}

fn remove_trigger_phrase(input: &str, keyword: &str) -> String {
    let keyword = keyword.trim();
    if keyword.is_empty() {
        return input.trim().to_string();
    }

    let pattern = if let Some((prefix, suffix)) = split_keyword(keyword) {
        build_trigger_pattern(prefix, suffix)
    } else {
        let keyword_pattern = normalize_for_pattern(keyword);
        if keyword_pattern.is_empty() {
            return input.trim().to_string();
        }
        format!("(?i){}", keyword_pattern)
    };
    let regex = Regex::new(&pattern).ok();
    let cleaned = regex
        .map(|re| re.replace(input, ""))
        .unwrap_or_else(|| input.into());
    cleaned.trim().to_string()
}

fn split_keyword(keyword: &str) -> Option<(&str, &str)> {
    let mut parts = keyword.split(VALUE_PLACEHOLDER);
    let prefix = parts.next()?;
    let suffix = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    Some((prefix, suffix))
}

fn match_sentence(sentence: &str, prefix: &str, suffix: &str) -> Option<String> {
    let pattern = build_trigger_pattern(prefix, suffix);
    let regex = Regex::new(&pattern).ok()?;
    let captures = regex.captures(sentence)?;
    let value = captures
        .name("value")
        .map(|value| value.as_str().to_string())?;
    Some(normalize_for_value(&value))
}

fn build_trigger_pattern(prefix: &str, suffix: &str) -> String {
    let prefix_pattern = normalize_for_pattern(prefix);
    let suffix_pattern = normalize_for_pattern(suffix);
    let value_pattern = if suffix_pattern.is_empty() {
        r"[^,，。！？!?.;；:：]+"
    } else {
        r"[^,，。！？!?.;；:：]+?"
    };
    format!(
        "(?i){}\\s*(?P<value>{})\\s*{}",
        prefix_pattern, value_pattern, suffix_pattern
    )
}

fn normalize_for_pattern(text: &str) -> String {
    let mut pattern = String::new();
    for ch in text.chars().filter_map(normalize_match_char) {
        if !pattern.is_empty() {
            pattern.push_str(r"\s*");
        }
        pattern.push_str(&regex::escape(&ch.to_string()));
    }
    pattern
}

fn normalize_for_compare(text: &str) -> String {
    text.chars().filter_map(normalize_match_char).collect()
}

fn normalize_for_value(value: &str) -> String {
    let trimmed = value.trim();
    let trimmed = trimmed.trim_matches(|ch: char| matches!(ch, '-' | '_' | '.' | ','));
    trimmed.trim().to_string()
}

fn normalize_match_char(ch: char) -> Option<char> {
    let normalized = match ch {
        '\u{3000}' => ' ',
        '\u{FF01}'..='\u{FF5E}' => char::from_u32((ch as u32).saturating_sub(0xFEE0)).unwrap_or(ch),
        _ => ch,
    };
    if normalized.is_whitespace() {
        None
    } else {
        Some(normalized.to_ascii_lowercase())
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn build_card(keyword: &str, variables: &[&str]) -> TriggerCard {
        TriggerCard {
            id: "test".to_string(),
            title: "Test".to_string(),
            enabled: true,
            auto_apply: false,
            locked: false,
            keyword: keyword.to_string(),
            prompt_template: "template {value}".to_string(),
            variables: variables.iter().map(|value| value.to_string()).collect(),
        }
    }

    #[test]
    fn split_sentences_supports_full_and_half_width_punctuation() {
        let sentences = split_sentences("请润色：这句话；翻译，英文。谢谢!");
        assert_eq!(sentences, vec!["请润色", "这句话", "翻译", "英文", "谢谢"]);
    }

    #[test]
    fn plain_keyword_matches_by_contains() {
        let card = build_card("润色", &["口语"]);
        let sentences = split_sentences("请帮我润色这句话");
        let matched = find_keyword_sentence(&card, &sentences);
        assert_eq!(matched, Some("请帮我润色这句话"));
    }

    #[test]
    fn variable_match_prefers_earliest_position() {
        let sentence = "请润色为书面，再补充口语版本";
        let matched = match_variable_in_sentence(
            sentence,
            &["口语".to_string(), "书面".to_string(), "书面版".to_string()],
        );
        assert_eq!(matched.as_deref(), Some("书面"));
    }

    #[test]
    fn match_card_falls_back_to_first_variable_when_missing() {
        let card = build_card("润色", &["口语", "书面"]);
        let sentences = split_sentences("请帮我润色一下");
        let matched = match_card(&card, &sentences);
        assert_eq!(matched, Some(("口语".to_string(), true)));
    }

    #[test]
    fn placeholder_keyword_is_still_supported() {
        let card = build_card("翻译为{value}", &["英文", "日文"]);
        let sentences = split_sentences("帮我翻译为日文");
        let matched = match_card(&card, &sentences);
        assert_eq!(matched, Some(("日文".to_string(), true)));
    }
}
