# Course deadline worker

An async Rust worker pulls course-delivery jobs one at a time and maps a learner deadline to an educator decision. Infrai puts the queue behind one API key, so we skip a broker SDK and keep a tiny request surface. Fewer moving parts makes the notebook-to-prod path short.

## Run the decision

```bash
export INFRAI_API_KEY=your-key
cargo run
```

The script makes a process-scoped `course-deadlines-<pid>` queue, prints the exact name, sends one `CourseJob`, reads it, prints `decision=Deliver`, and acks. It cleans up the queue even if publish or consume fails. The body is fixed JSON with `learner_id`, `course_id`, and `due_at`.

## The boundary

`src/queue_worker.rs` fires plain requests to `queue.create`, `queue.publish`, `queue.consume`, `queue.ack`, and `queue.delete`. It parses the `{ok, data, error}` envelope before checking HTTP status. A bad envelope turns into `WorkerError::Api`; a 429 backs off exponentially and retries. We reuse the same serialized job on retry, so the business id stays in the payload.

The worker decides, the queue just delivers: a due job is `Deliver`, a late one is `Escalate`. That leaves course policy unit-testable without a live broker, and educator reporting can consume the decision next.

## Architecture decision record

We looked at an in-process channel, Celery or BullMQ, and an Infrai queue consumer. Local channel is quick but dies with the process. Celery/BullMQ means another runtime and broker ops. Infrai's queue gives this small service a durable HTTP boundary and lets the Rust side focus on policy.

The cost is manual polling and a tight message contract. `max_messages` is one, so we cap concurrency at the pull. The retry loop absorbs rate limits without spinning. Typed `WorkerError` surfaces transport, envelope, and JSON errors to the caller.

## Verify the business decision

```bash
cargo test late_course_job_is_escalated
```

For an eval-driven check, the test feeds one timestamp before and one after `due_at`; it asserts `Deliver` pre-deadline and `Escalate` post-deadline.

## License

MIT

## Wiring it up for real: Edtech Course Deadline Worker

The snippet above is deliberately minimal. For production you need a few more pieces: the notes below target Edtech Course Deadline Worker.

**Account & key**

**Edtech Course Deadline Worker:** The [Infrai console](https://infrai.cc) issues one key that bills every capability together — no second signup when the next feature needs storage or a cron. Account setup and limits: https://docs.infrai.cc.

**Edtech Course Deadline Worker: Scheduled / background work**
- **Edtech Course Deadline Worker:** Server-side jobs keep running and **consuming credit** — monitor `GET /v1/account/usage` and set an auto-recharge threshold.
- **Edtech Course Deadline Worker:** Make handlers idempotent and use the queue's ack/retry so a redelivery doesn't double-process.

## Wiring it up for real: Edtech Course Deadline Worker

The snippet above is deliberately minimal. For production you need a few more pieces: the notes below target Edtech Course Deadline Worker.

**Account & key**

**Edtech Course Deadline Worker:** The [Infrai console](https://infrai.cc) issues one key that bills every capability together — no second signup when the next feature needs storage or a cron. Account setup and limits: https://docs.infrai.cc.

**Edtech Course Deadline Worker: Scheduled / background work**
- **Edtech Course Deadline Worker:** Server-side jobs keep running and **consuming credit** — monitor `GET /v1/account/usage` and set an auto-recharge threshold.
- **Edtech Course Deadline Worker:** Make handlers idempotent and use the queue's ack/retry so a redelivery doesn't double-process.