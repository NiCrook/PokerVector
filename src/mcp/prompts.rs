use rmcp::model::*;

use super::PokerVectorMcp;

impl PokerVectorMcp {
    pub(crate) fn list_prompt_entries(&self) -> Vec<Prompt> {
        vec![
            Prompt::new(
                "review-last-session",
                Some("Review your most recent poker session — stats, notable hands, and mistakes"),
                None,
            ),
            Prompt::new(
                "analyze-villain",
                Some("Deep analysis of a specific opponent's play style with exploit suggestions"),
                Some(vec![PromptArgument {
                    name: "villain".to_string(),
                    title: None,
                    description: Some("Villain name to analyze".to_string()),
                    required: Some(true),
                }]),
            ),
            Prompt::new(
                "find-my-leaks",
                Some("Identify your biggest leaks by comparing stats against healthy baselines"),
                None,
            ),
        ]
    }

    pub(crate) fn get_prompt_by_name(
        &self,
        name: &str,
        arguments: &Option<serde_json::Map<String, serde_json::Value>>,
    ) -> Result<GetPromptResult, ErrorData> {
        match name {
            "review-last-session" => Ok(self.prompt_review_last_session()),
            "analyze-villain" => {
                let villain = arguments
                    .as_ref()
                    .and_then(|args| args.get("villain"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        ErrorData::invalid_params(
                            "Missing required argument: villain".to_string(),
                            None,
                        )
                    })?;
                Ok(self.prompt_analyze_villain(villain))
            }
            "find-my-leaks" => Ok(self.prompt_find_my_leaks()),
            _ => Err(ErrorData::invalid_params(
                format!("Unknown prompt: {}", name),
                None,
            )),
        }
    }

    fn prompt_review_last_session(&self) -> GetPromptResult {
        GetPromptResult {
            description: Some("Review the most recent poker session".to_string()),
            messages: vec![PromptMessage::new_text(
                PromptMessageRole::User,
                format!(
                    "Review my most recent poker session. I play as \"{}\".\n\n\
                     Please:\n\
                     1. Use list_sessions to find my latest session\n\
                     2. Use review_session on that session for the full breakdown\n\
                     3. For any notable hands (biggest wins/losses), use get_hand to see the full details\n\
                     4. Identify any mistakes or questionable plays\n\
                     5. Summarize my session performance — what went well and what I should work on",
                    self.hero,
                ),
            )],
        }
    }

    fn prompt_analyze_villain(&self, villain: &str) -> GetPromptResult {
        GetPromptResult {
            description: Some(format!("Analyze opponent: {}", villain)),
            messages: vec![PromptMessage::new_text(
                PromptMessageRole::User,
                format!(
                    "Give me a complete analysis of the opponent \"{}\" from my poker database. I play as \"{}\".\n\n\
                     Please:\n\
                     1. Use get_villain_profile to get their full stats, showdown hands, and positional breakdown\n\
                     2. Use get_villain_tendencies to see how they react to c-bets, barrels, and other lines\n\
                     3. Use get_sizing_profile with player=\"{}\" to analyze their bet sizing patterns\n\
                     4. Use get_positional_matchups to see where I do well/poorly against them\n\
                     5. Based on all this data, classify their play style (nit/TAG/LAG/fish/maniac)\n\
                     6. Give me specific, actionable exploits for playing against this opponent",
                    villain, self.hero, villain,
                ),
            )],
        }
    }

    fn prompt_find_my_leaks(&self) -> GetPromptResult {
        GetPromptResult {
            description: Some("Find leaks in hero's game".to_string()),
            messages: vec![PromptMessage::new_text(
                PromptMessageRole::User,
                format!(
                    "Analyze my poker game and find my biggest leaks. I play as \"{}\".\n\n\
                     Please:\n\
                     1. Use find_leaks to compare my stats against healthy baselines\n\
                     2. Use get_stats to get my overall stat profile\n\
                     3. Use get_street_stats to check my per-street tendencies\n\
                     4. Use get_trends (period='week') to see if any leaks are getting worse over time\n\
                     5. For each major leak found, search for 2-3 example hands that demonstrate the problem\n\
                     6. Rank the leaks by impact (which ones cost me the most money)\n\
                     7. Give specific, prioritized recommendations for improvement",
                    self.hero,
                ),
            )],
        }
    }
}
