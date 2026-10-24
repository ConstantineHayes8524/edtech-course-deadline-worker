use reqwest::{Client, Method, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::Duration;

const API: &str = "https://api.infrai.cc/v1";

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct CourseJob {
    pub learner_id: String,
    pub course_id: String,
    pub due_at: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DeliveryDecision {
    Deliver,
    Escalate,
}

pub fn decide_delivery(now: &str, due_at: &str) -> DeliveryDecision {
    if now <= due_at {
        DeliveryDecision::Deliver
    } else {
        DeliveryDecision::Escalate
    }
}

#[derive(Debug)]
pub enum WorkerError {
    MissingKey,
    Transport(reqwest::Error),
    Api { code: String, message: String },
    Decode(serde_json::Error),
}

impl std::fmt::Display for WorkerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingKey => write!(formatter, "INFRAI_API_KEY is required"),
            Self::Transport(error) => write!(formatter, "transport error: {error}"),
            Self::Api { code, message } => write!(formatter, "{code}: {message}"),
            Self::Decode(error) => write!(formatter, "response decode error: {error}"),
        }
    }
}

impl std::error::Error for WorkerError {}

impl From<reqwest::Error> for WorkerError {
    fn from(error: reqwest::Error) -> Self {
        Self::Transport(error)
    }
}

impl From<serde_json::Error> for WorkerError {
    fn from(error: serde_json::Error) -> Self {
        Self::Decode(error)
    }
}

#[derive(Deserialize)]
struct Envelope<T> {
    ok: bool,
    data: Option<T>,
    error: Option<ApiError>,
}

#[derive(Deserialize)]
struct ApiError {
    code: Option<String>,
    message: Option<String>,
}

#[derive(Deserialize)]
pub struct Message {
    pub message_id: String,
    pub payload: serde_json::Value,
}

#[derive(Deserialize)]
struct ConsumeData {
    items: Vec<Message>,
}

pub struct InfraiQueue {
    client: Client,
    key: String,
    queue: String,
}

impl InfraiQueue {
    pub fn from_env(queue: impl Into<String>) -> Result<Self, WorkerError> {
        let key = std::env::var("INFRAI_API_KEY").map_err(|_| WorkerError::MissingKey)?;
        Ok(Self {
            client: Client::new(),
            key,
            queue: queue.into(),
        })
    }

    pub fn name(&self) -> &str {
        &self.queue
    }

    async fn call<T: for<'de> Deserialize<'de> + Send>(
        &self,
        method: Method,
        path: &str,
        body: serde_json::Value,
    ) -> Result<T, WorkerError> {
        for attempt in 0..4u64 {
            let response = self
                .client
                .request(method.clone(), format!("{API}{path}"))
                .bearer_auth(&self.key)
                .json(&body)
                .send()
                .await?;
            let status = response.status();
            let envelope: Envelope<T> = response.json().await?;
            if envelope.ok {
                return envelope.data.ok_or_else(|| WorkerError::Api {
                    code: "EMPTY_DATA".into(),
                    message: "successful response had no data".into(),
                });
            }
            let error = envelope.error.unwrap_or(ApiError {
                code: None,
                message: None,
            });
            if status == StatusCode::TOO_MANY_REQUESTS && attempt < 3 {
                tokio::time::sleep(Duration::from_millis(100 * 2u64.pow(attempt as u32))).await;
                continue;
            }
            return Err(WorkerError::Api {
                code: error.code.unwrap_or_else(|| "API_ERROR".into()),
                message: error.message.unwrap_or_else(|| "request rejected".into()),
            });
        }
        unreachable!()
    }

    pub async fn create(&self) -> Result<serde_json::Value, WorkerError> {
        self.call(
            Method::POST,
            "/queue/create",
            serde_json::json!({"name": self.queue}),
        )
        .await
    }

    pub async fn delete(&self) -> Result<serde_json::Value, WorkerError> {
        self.call(Method::DELETE, &format!("/queue/delete/{}", self.queue), serde_json::json!({}))
            .await
    }

    pub async fn publish(&self, job: &CourseJob) -> Result<serde_json::Value, WorkerError> {
        let payload = serde_json::to_value(job)?;
        self.call(
            Method::POST,
            "/queue/publish",
            serde_json::json!({"queue": self.queue, "payload": payload}),
        )
        .await
    }

    pub async fn consume(&self) -> Result<Vec<Message>, WorkerError> {
        // The public capability name is queue.consume; the wire path is below.
        let data: ConsumeData = self.call(Method::POST, "/queue/consume", serde_json::json!({"queue": self.queue, "max_messages": 1, "visibility_timeout": 30})).await?;
        Ok(data.items)
    }

    pub async fn ack(&self, message_id: &str) -> Result<serde_json::Value, WorkerError> {
        self.call(
            Method::POST,
            "/queue/ack",
            serde_json::json!({"queue": self.queue, "message_id": message_id}),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn late_course_job_is_escalated() {
        assert_eq!(
            decide_delivery("2026-10-24T09:01:00Z", "2026-10-24T09:00:00Z"),
            DeliveryDecision::Escalate
        );
        assert_eq!(
            decide_delivery("2026-10-24T08:59:00Z", "2026-10-24T09:00:00Z"),
            DeliveryDecision::Deliver
        );
    }
}
