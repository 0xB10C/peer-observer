pub struct NatsPublisherForTesting {
    client: async_nats::Client,
}

impl NatsPublisherForTesting {
    pub async fn new(port: u16) -> Self {
        Self {
            client: async_nats::connect(format!("127.0.0.1:{}", port))
                .await
                .expect("should be able to connect to NATS server"),
        }
    }

    pub async fn publish(&self, subject: String, payload: Vec<u8>) {
        self.client
            .publish(subject, payload.into())
            .await
            .expect("should be able to publish");
    }
}
