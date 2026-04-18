use crate::{DeliveryError, DeliveryReport};

pub trait ProducerCallback: Send + Sync + 'static {
    fn on_delivery(&self, _report: &DeliveryReport) {}
    fn on_error(&self, _error: &DeliveryError) {}
}
