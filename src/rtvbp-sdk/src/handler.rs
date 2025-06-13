use rtvbp_spec::v1::message::Message;
use tokio::sync::mpsc::Sender;

#[derive(Debug)]
pub enum HandlerError {}

#[derive(Debug)]
pub struct MessageContext {
    tx: Sender<Message>,
    msg: Message,
}

impl MessageContext {
    pub fn msg(&self) -> &Message {
        &self.msg
    }

    pub async fn send(&self, msg: Message) -> Result<(), HandlerError> {
        self.tx.send(msg).await.unwrap();
        Ok(())
    }
}

impl MessageContext {
    pub fn new(tx: Sender<Message>, msg: Message) -> Self {
        Self { tx, msg }
    }
}
