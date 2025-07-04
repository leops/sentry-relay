// This module only defines traits, every parameter is used by definition
#![allow(unused_variables)]

use std::fmt::Debug;

use relay_protocol::{FiniteF64, FromValue, IntoValue, Meta};

use crate::processor::{ProcessingState, ValueType, process_value};

/// Used to indicate how to handle an annotated value in a callback.
#[must_use = "This `ProcessingAction` must be handled by `Annotated::apply`"]
#[derive(Copy, Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ProcessingAction {
    /// Discards the value entirely.
    #[error("value should be hard-deleted (unreachable, should not surface as error!)")]
    DeleteValueHard,

    /// Discards the value and moves it into meta's `original_value`.
    #[error("value should be hard-deleted (unreachable, should not surface as error!)")]
    DeleteValueSoft,

    /// The event is invalid (needs to bubble up)
    #[error("invalid transaction event: {0}")]
    InvalidTransaction(&'static str),
}

/// The result of running a processor on a value implementing `ProcessValue`.
pub type ProcessingResult = Result<(), ProcessingAction>;

macro_rules! process_method {
    ($name: ident, $ty:ident $(::$path:ident)*) => {
        process_method!($name, $ty $(::$path)* <>);
    };

    ($name: ident, $ty:ident $(::$path:ident)* < $($param:ident),* > $(, $param_req_key:ident : $param_req_trait:path)*) => {
        #[inline]
        fn $name<$($param),*>(
            &mut self,
            value: &mut $ty $(::$path)* <$($param),*>,
            meta: &mut Meta,
            state: &ProcessingState<'_>,
        ) -> ProcessingResult
        where
            $($param: ProcessValue),*
            $(, $param_req_key : $param_req_trait)*
        {
            value.process_child_values(self, state)?;
            Ok(())
        }
    };
}

/// A trait for processing processable values.
pub trait Processor: Sized {
    #[inline]
    fn before_process<T: ProcessValue>(
        &mut self,
        value: Option<&T>,
        meta: &mut Meta,
        state: &ProcessingState<'_>,
    ) -> ProcessingResult {
        Ok(())
    }

    #[inline]
    fn after_process<T: ProcessValue>(
        &mut self,
        value: Option<&T>,
        meta: &mut Meta,
        state: &ProcessingState<'_>,
    ) -> ProcessingResult {
        Ok(())
    }

    process_method!(process_string, String);
    process_method!(process_u64, u64);
    process_method!(process_i64, i64);
    process_method!(process_f64, f64);
    process_method!(process_finite_f64, FiniteF64);
    process_method!(process_bool, bool);

    process_method!(process_value, relay_protocol::Value);
    process_method!(process_array, relay_protocol::Array<T>);
    process_method!(process_object, relay_protocol::Object<T>);

    process_method!(
        process_pairlist,
        crate::protocol::PairList<T>,
        T: crate::protocol::AsPair
    );
    process_method!(process_values, crate::protocol::Values<T>);
    process_method!(process_timestamp, crate::protocol::Timestamp);

    process_method!(process_event, crate::protocol::Event);
    process_method!(process_replay, crate::protocol::Replay);
    process_method!(process_exception, crate::protocol::Exception);
    process_method!(process_raw_stacktrace, crate::protocol::RawStacktrace);
    process_method!(process_stacktrace, crate::protocol::Stacktrace);
    process_method!(process_frame, crate::protocol::Frame);
    process_method!(process_request, crate::protocol::Request);
    process_method!(process_user, crate::protocol::User);
    process_method!(process_client_sdk_info, crate::protocol::ClientSdkInfo);
    process_method!(process_debug_meta, crate::protocol::DebugMeta);
    process_method!(process_debug_image, crate::protocol::DebugImage);
    process_method!(process_geo, crate::protocol::Geo);
    process_method!(process_logentry, crate::protocol::LogEntry);
    process_method!(process_thread, crate::protocol::Thread);
    process_method!(process_context, crate::protocol::Context);
    process_method!(process_breadcrumb, crate::protocol::Breadcrumb);
    process_method!(process_template_info, crate::protocol::TemplateInfo);
    process_method!(process_header_name, crate::protocol::HeaderName);
    process_method!(process_ourlog, crate::protocol::OurLog);
    process_method!(process_span, crate::protocol::Span);
    process_method!(process_trace_context, crate::protocol::TraceContext);
    process_method!(process_native_image_path, crate::protocol::NativeImagePath);
    process_method!(process_contexts, crate::protocol::Contexts);

    fn process_other(
        &mut self,
        other: &mut relay_protocol::Object<relay_protocol::Value>,
        state: &ProcessingState<'_>,
    ) -> ProcessingResult {
        for (key, value) in other {
            process_value(
                value,
                self,
                &state.enter_borrowed(
                    key.as_str(),
                    state.inner_attrs(),
                    ValueType::for_field(value),
                ),
            )?;
        }

        Ok(())
    }
}

#[doc(inline)]
pub use enumset::{EnumSet, enum_set};

/// A recursively processable value.
pub trait ProcessValue: FromValue + IntoValue + Debug + Clone {
    /// Returns the type of the value.
    #[inline]
    fn value_type(&self) -> EnumSet<ValueType> {
        EnumSet::empty()
    }

    /// Executes a processor on this value.
    #[inline]
    fn process_value<P>(
        &mut self,
        meta: &mut Meta,
        processor: &mut P,
        state: &ProcessingState<'_>,
    ) -> ProcessingResult
    where
        P: Processor,
    {
        self.process_child_values(processor, state)
    }

    /// Recurses into children of this value.
    #[inline]
    fn process_child_values<P>(
        &mut self,
        processor: &mut P,
        state: &ProcessingState<'_>,
    ) -> ProcessingResult
    where
        P: Processor,
    {
        Ok(())
    }
}

pub use relay_event_derive::ProcessValue;
