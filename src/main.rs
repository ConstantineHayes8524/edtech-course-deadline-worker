mod queue_worker;

use queue_worker::{decide_delivery, CourseJob, InfraiQueue};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let queue = format!("course-deadlines-{}", std::process::id());
    let worker = InfraiQueue::from_env(queue)?;
    worker.create().await?;
    println!("created queue={}", worker.name());
    let run = async {
        worker.publish(&CourseJob {
            learner_id: "learner-42".into(),
            course_id: "rust-101".into(),
            due_at: "2026-10-24T09:00:00Z".into(),
        })
        .await?;
        for message in worker.consume().await? {
            let job: CourseJob = serde_json::from_value(message.payload)?;
            let decision = decide_delivery("2026-10-24T08:30:00Z", &job.due_at);
            println!(
                "learner={} course={} decision={decision:?}",
                job.learner_id, job.course_id
            );
            worker.ack(&message.message_id).await?;
        }
        Ok::<(), Box<dyn std::error::Error>>(())
    }
    .await;
    let cleanup = worker.delete().await;
    run?;
    cleanup?;
    println!("deleted queue={}", worker.name());
    Ok(())
}
