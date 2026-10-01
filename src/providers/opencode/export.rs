use std::collections::BTreeMap;

use serde::Deserialize;

use super::super::{ModelUsage, ProviderError, ProviderUsage, UsageWindow, WindowKind};

#[derive(Debug, Deserialize)]
struct Record {
    provider: Option<String>,
    model: Option<String>,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
    cost_micro_cents: Option<u64>,
    service: Option<String>,
}

#[derive(Debug, Default)]
struct Counters {
    input: Option<u64>,
    output: Option<u64>,
    cached: Option<u64>,
    cost: Option<u64>,
    requests: u64,
}

impl Counters {
    fn add(&mut self, record: &Record) -> Result<(), ProviderError> {
        self.input = sum(self.input, record.input_tokens)?;
        self.output = sum(self.output, record.output_tokens)?;
        self.cached = sum(self.cached, record.cache_read_tokens)?;
        self.cost = sum(self.cost, record.cost_micro_cents)?;
        self.requests = self
            .requests
            .checked_add(1)
            .ok_or(ProviderError::InvalidData)?;
        Ok(())
    }

    fn model_usage(self, model: String) -> ModelUsage {
        ModelUsage {
            model,
            input_tokens: self.input,
            cached_input_tokens: self.cached,
            output_tokens: self.output,
            requests: Some(self.requests),
            cost_usd: self.cost.map(micro_cents_to_usd),
        }
    }
}

fn sum(total: Option<u64>, value: Option<u64>) -> Result<Option<u64>, ProviderError> {
    match (total, value) {
        (Some(total), Some(value)) => total
            .checked_add(value)
            .map(Some)
            .ok_or(ProviderError::InvalidData),
        (None, Some(value)) => Ok(Some(value)),
        (total, None) => Ok(total),
    }
}

fn micro_cents_to_usd(value: u64) -> f64 {
    value as f64 / 100_000_000.0
}

pub(super) fn normalize(body: &[u8]) -> Result<ProviderUsage, ProviderError> {
    let mut csv = csv::ReaderBuilder::new().from_reader(body);
    let headers = csv.headers().map_err(|_| ProviderError::InvalidData)?;
    if ![
        "id",
        "created_at",
        "provider",
        "model",
        "input_tokens",
        "output_tokens",
        "cache_read_tokens",
        "cost_micro_cents",
        "service",
    ]
    .iter()
    .all(|field| headers.iter().any(|header| header == *field))
    {
        return Err(ProviderError::InvalidData);
    }

    let mut totals = Counters::default();
    let mut models = BTreeMap::<String, Counters>::new();
    for result in csv.deserialize::<Record>() {
        let record = result.map_err(|_| ProviderError::InvalidData)?;
        if record
            .service
            .as_deref()
            .is_some_and(|service| !service.is_empty())
        {
            continue;
        }
        totals.add(&record)?;
        if let (Some(provider), Some(model)) = (&record.provider, &record.model) {
            models
                .entry(format!("{provider}/{model}"))
                .or_default()
                .add(&record)?;
        }
    }
    let window = UsageWindow {
        kind: WindowKind::Custom,
        label: "Workspace export (7d UTC, all providers)".to_owned(),
        duration_seconds: None,
        used_percent: None,
        remaining_percent: None,
        resets_at: None,
        tokens: None,
        requests: Some(totals.requests),
        cost_usd: totals.cost.map(micro_cents_to_usd),
    };
    Ok(ProviderUsage {
        provider_id: "opencode-go".to_owned(),
        display_name: "OpenCode Go".to_owned(),
        account_label: Some("OpenCode workspace (all providers)".to_owned()),
        windows: vec![window],
        totals: Some(totals.model_usage("all".to_owned())),
        models: Some(
            models
                .into_iter()
                .map(|(model, counters)| counters.model_usage(model))
                .collect(),
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = "id,created_at,provider,model,input_tokens,output_tokens,cache_read_tokens,cost_micro_cents,service\n";

    #[test]
    fn header_only_export_has_no_invented_quota() {
        let usage = normalize(HEADER.as_bytes()).expect("empty export");
        assert_eq!(usage.windows[0].requests, Some(0));
        assert_eq!(usage.windows[0].cost_usd, None);
        assert_eq!(usage.windows[0].used_percent, None);
        assert_eq!(usage.windows[0].resets_at, None);
    }

    #[test]
    fn partial_cells_remain_absent() {
        let body = format!("{HEADER}1,2026-09-30T12:00:00Z,anthropic,claude-sonnet,,,,,\n");
        let usage = normalize(body.as_bytes()).expect("partial export");
        let totals = usage.totals.expect("totals");
        assert_eq!(totals.requests, Some(1));
        assert_eq!(totals.input_tokens, None);
        assert_eq!(totals.cost_usd, None);
    }

    #[test]
    fn rejects_wrong_schema_or_malformed_numbers() {
        for body in [
            "not csv",
            "id,created_at,provider,model,service\n1,now,a,m,\n",
            "id,created_at,provider,model,input_tokens,output_tokens,cache_read_tokens,cost_micro_cents,service\n1,now,a,m,wrong,,,,\n",
        ] {
            assert!(normalize(body.as_bytes()).is_err());
        }
    }
}
