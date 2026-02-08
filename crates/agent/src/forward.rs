use serde::Serialize;

use core::tunnel::TunnelSignal;

pub struct Forwarder {
    client: reqwest::Client,
    forward_url: String,
}

impl Forwarder {
    pub fn new(forward_url: String) -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        Ok(Self { client, forward_url })
    }

    pub async fn deliver_signal(
        &self,
        delivery_id: &str,
        channel_id: &str,
        channel_slug: &str,
        signal: &TunnelSignal,
    ) -> anyhow::Result<()> {
        let payload = ForwardPayload {
            delivery_id,
            channel_id,
            channel_slug,
            signal,
        };

        let resp = self
            .client
            .post(&self.forward_url)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        if resp.status().is_success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!("forward failed: HTTP {}", resp.status()))
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ForwardPayload<'a> {
    delivery_id: &'a str,
    channel_id: &'a str,
    channel_slug: &'a str,
    signal: &'a TunnelSignal,
}
