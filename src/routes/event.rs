use crate::app::AppState;
use async_stream::try_stream;
use axum::{
    extract::State,
    response::{
        sse::{Event, KeepAlive},
        Sse,
    },
};
use futures::Stream;
use std::{convert::Infallible, sync::Arc};

pub async fn stream(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    tracing::info!("new connection to sse stream >>>");

    let mut rx = state.get_sender().subscribe();

    Sse::new(try_stream! {
        loop {
            match rx.recv().await {
                Ok(i) => {
                    let event = Event::default().data(serde_json::to_string(&i).unwrap());

                    yield event;
                }

                Err(e) => {
                    tracing::error!(error = ?e, "Failed to get");
                }
            }
        }
    })
    .keep_alive(KeepAlive::default())
}
