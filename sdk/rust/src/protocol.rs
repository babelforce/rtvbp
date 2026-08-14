use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;

/// Structured validation emitted from catalog metadata.
pub trait Validate {
    /// Validate catalog-defined semantic constraints.
    ///
    /// # Errors
    ///
    /// Returns a structured validation error when a constraint is violated.
    fn validate(&self) -> Result<(), crate::ValidationError> {
        Ok(())
    }
}

/// A generated named request and its typed response.
pub trait NamedRequest: Serialize + Validate + Send + Sync {
    type Response: DeserializeOwned + Serialize + Validate + Send + Sync;
    const METHOD: &'static str;
}

/// A generated named event.
pub trait NamedEvent: Serialize + Validate + Send + Sync {
    const EVENT: &'static str;
}

/// Narrow raw request capability used by generated typed peers.
#[async_trait]
pub trait Requester: Send + Sync {
    async fn request(&self, method: &'static str, payload: Value) -> Result<Value, crate::Error>;
}

/// Narrow raw event capability used by generated event emitters.
#[async_trait]
pub trait Notifier: Send + Sync {
    async fn notify(&self, event: &'static str, payload: Value) -> Result<(), crate::Error>;
}

/// Validate and issue a generated typed request.
///
/// # Errors
///
/// Returns validation, encoding, transport, remote, or response-decoding failures.
pub async fn request_peer<R, Q>(requester: &R, request: Q) -> Result<Q::Response, crate::Error>
where
    R: Requester,
    Q: NamedRequest,
{
    request.validate()?;
    let payload = serde_json::to_value(request).map_err(crate::Error::envelope)?;
    let response = requester.request(Q::METHOD, payload).await?;
    let response = if response.is_null() {
        Value::Object(serde_json::Map::new())
    } else {
        response
    };
    let response: Q::Response = serde_json::from_value(response).map_err(crate::Error::envelope)?;
    response.validate()?;
    Ok(response)
}

/// Validate and issue a generated typed event.
///
/// # Errors
///
/// Returns validation, encoding, or transport failures.
pub async fn notify_event<N, E>(notifier: &N, event: E) -> Result<(), crate::Error>
where
    N: Notifier,
    E: NamedEvent,
{
    event.validate()?;
    let payload = serde_json::to_value(event).map_err(crate::Error::envelope)?;
    notifier.notify(E::EVENT, payload).await
}

/// One successful handler result plus its terminal-session policy.
#[derive(Clone, Debug, PartialEq)]
pub struct HandlerReply {
    pub payload: Value,
    pub terminal: bool,
}

type RequestFuture = Pin<Box<dyn Future<Output = Result<HandlerReply, crate::Error>> + Send>>;
type RequestCallback = dyn Fn(crate::HandlerContext, Value) -> RequestFuture + Send + Sync;

/// A generated request dispatch registration.
pub struct RequestRegistration {
    method: &'static str,
    callback: Arc<RequestCallback>,
}

impl RequestRegistration {
    pub fn typed<Req, Res, F, Fut>(method: &'static str, terminal: bool, callback: F) -> Self
    where
        Req: DeserializeOwned + Validate + Send + 'static,
        Res: Serialize + Validate + Send + 'static,
        F: Fn(crate::HandlerContext, Req) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Res, crate::Error>> + Send + 'static,
    {
        Self {
            method,
            callback: Arc::new(move |context, payload| {
                let request =
                    serde_json::from_value::<Req>(payload).map_err(crate::Error::envelope);
                let future = match request {
                    Ok(request) => {
                        if let Err(error) = request.validate() {
                            return Box::pin(async move { Err(error.into()) }) as RequestFuture;
                        }
                        callback(context, request)
                    }
                    Err(error) => return Box::pin(async move { Err(error) }) as RequestFuture,
                };
                Box::pin(async move {
                    let response = future.await?;
                    response.validate()?;
                    Ok(HandlerReply {
                        payload: serde_json::to_value(response).map_err(crate::Error::envelope)?,
                        terminal,
                    })
                })
            }),
        }
    }

    #[must_use]
    pub fn rejection(method: &'static str, error: crate::WireError) -> Self {
        let error = Arc::new(error);
        Self {
            method,
            callback: Arc::new(move |_, _| {
                let error = Arc::clone(&error);
                Box::pin(async move { Err(crate::Error::Handler((*error).clone())) })
            }),
        }
    }

    #[must_use]
    pub const fn method(&self) -> &'static str {
        self.method
    }

    /// Invoke the registered typed handler.
    ///
    /// # Errors
    ///
    /// Returns decoding, validation, or handler failures.
    pub async fn handle(
        &self,
        context: crate::HandlerContext,
        payload: Value,
    ) -> Result<HandlerReply, crate::Error> {
        (self.callback)(context, payload).await
    }
}

type EventFuture = Pin<Box<dyn Future<Output = Result<(), crate::Error>> + Send>>;
type EventCallback = dyn Fn(crate::HandlerContext, Value) -> EventFuture + Send + Sync;

/// A generated event dispatch registration.
pub struct EventRegistration {
    event: &'static str,
    callback: Arc<EventCallback>,
}

impl EventRegistration {
    pub fn typed<E, F, Fut>(event: &'static str, callback: F) -> Self
    where
        E: DeserializeOwned + Validate + Send + 'static,
        F: Fn(crate::HandlerContext, E) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), crate::Error>> + Send + 'static,
    {
        Self {
            event,
            callback: Arc::new(move |context, payload| {
                let event = serde_json::from_value::<E>(payload).map_err(crate::Error::envelope);
                let event = match event {
                    Ok(event) => event,
                    Err(error) => return Box::pin(async move { Err(error) }) as EventFuture,
                };
                if let Err(error) = event.validate() {
                    return Box::pin(async move { Err(error.into()) }) as EventFuture;
                }
                Box::pin(callback(context, event))
            }),
        }
    }

    #[must_use]
    pub const fn event(&self) -> &'static str {
        self.event
    }

    /// Invoke the registered typed event handler.
    ///
    /// # Errors
    ///
    /// Returns decoding, validation, or handler failures.
    pub async fn handle(
        &self,
        context: crate::HandlerContext,
        payload: Value,
    ) -> Result<(), crate::Error> {
        (self.callback)(context, payload).await
    }
}
