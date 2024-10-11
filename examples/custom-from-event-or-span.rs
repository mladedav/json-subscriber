use std::fmt::Debug;

use tracing::{
    field::Visit,
    span::{Attributes, Id, Record},
    Subscriber,
};
use tracing_core::Field;
use tracing_subscriber::{
    layer::{Context, SubscriberExt},
    registry::LookupSpan,
    util::SubscriberInitExt,
    Layer,
    Registry,
};

fn main() {
    let mut json_layer = json_subscriber::layer();
    json_layer
        .inner_layer_mut()
        .add_dynamic_field("app_id", |event, ctx| {
            let mut app_id = None;
            event.record(&mut AppIdVisitor(&mut app_id));
            if let Some(app_id) = app_id {
                return Some(app_id);
            }

            for span in ctx.event_scope(event)? {
                if let Some(app_id) = span.extensions().get::<AppId>() {
                    return Some(app_id.0.clone());
                }
            }

            None
        });

    Registry::default().with(json_layer).with(AppIdLayer).init();

    tracing::info!("Normal log without app_id.");
    tracing::info!(app_id = 7, "Log with app_id.");
    let _span = tracing::info_span!("operation", app_id = "from_span").entered();
    tracing::info!("Log with inherited app_id.");
    tracing::info!(app_id = "from_event", "Log with overridden app_id.");
}

struct AppId(String);

struct AppIdLayer;

impl<S: Subscriber + for<'lookup> LookupSpan<'lookup>> Layer<S> for AppIdLayer {
    fn on_new_span(&self, attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let mut app_id = None;
        attrs.record(&mut AppIdVisitor(&mut app_id));
        if let Some(app_id) = app_id {
            let span = ctx.span(id).expect("This span was just created.");
            span.extensions_mut().insert(AppId(app_id));
        }
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        let mut app_id = None;
        values.record(&mut AppIdVisitor(&mut app_id));
        if let Some(app_id) = app_id {
            let span = ctx.span(id).expect("This span was just created.");
            span.extensions_mut().replace(AppId(app_id));
        }
    }
}

struct AppIdVisitor<'a>(&'a mut Option<String>);

impl<'a> Visit for AppIdVisitor<'a> {
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        if field.name() == "app_id" {
            *self.0 = Some(format!("{value:?}"));
        }
    }
}
