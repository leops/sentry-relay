use std::borrow::Cow;

use enumset::EnumSet;
use relay_protocol::{Annotated, Array, FiniteF64, Meta, Object, Value};
use uuid::Uuid;

use crate::processor::{
    ProcessValue, ProcessingResult, ProcessingState, Processor, ValueType, process_value,
};

impl ProcessValue for String {
    #[inline]
    fn value_type(&self) -> EnumSet<ValueType> {
        EnumSet::only(ValueType::String)
    }

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
        processor.process_string(self, meta, state)
    }
}

impl ProcessValue for bool {
    #[inline]
    fn value_type(&self) -> EnumSet<ValueType> {
        EnumSet::only(ValueType::Boolean)
    }

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
        processor.process_bool(self, meta, state)
    }
}

impl ProcessValue for u64 {
    #[inline]
    fn value_type(&self) -> EnumSet<ValueType> {
        EnumSet::only(ValueType::Number)
    }

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
        processor.process_u64(self, meta, state)
    }
}

impl ProcessValue for i64 {
    #[inline]
    fn value_type(&self) -> EnumSet<ValueType> {
        EnumSet::only(ValueType::Number)
    }

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
        processor.process_i64(self, meta, state)
    }
}

impl ProcessValue for f64 {
    #[inline]
    fn value_type(&self) -> EnumSet<ValueType> {
        EnumSet::only(ValueType::Number)
    }

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
        processor.process_f64(self, meta, state)
    }
}

impl ProcessValue for FiniteF64 {
    #[inline]
    fn value_type(&self) -> EnumSet<ValueType> {
        EnumSet::only(ValueType::Number)
    }

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
        processor.process_finite_f64(self, meta, state)
    }
}

impl ProcessValue for Uuid {}

impl<T> ProcessValue for Array<T>
where
    T: ProcessValue,
{
    #[inline]
    fn value_type(&self) -> EnumSet<ValueType> {
        EnumSet::only(ValueType::Array)
    }

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
        processor.process_array(self, meta, state)
    }

    #[inline]
    fn process_child_values<P>(
        &mut self,
        processor: &mut P,
        state: &ProcessingState<'_>,
    ) -> ProcessingResult
    where
        P: Processor,
    {
        for (index, element) in self.iter_mut().enumerate() {
            process_value(
                element,
                processor,
                &state.enter_index(index, state.inner_attrs(), ValueType::for_field(element)),
            )?;
        }

        Ok(())
    }
}

impl<T> ProcessValue for Object<T>
where
    T: ProcessValue,
{
    #[inline]
    fn value_type(&self) -> EnumSet<ValueType> {
        EnumSet::only(ValueType::Object)
    }

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
        processor.process_object(self, meta, state)
    }

    #[inline]
    fn process_child_values<P>(
        &mut self,
        processor: &mut P,
        state: &ProcessingState<'_>,
    ) -> ProcessingResult
    where
        P: Processor,
    {
        for (k, v) in self.iter_mut() {
            process_value(
                v,
                processor,
                &state.enter_borrowed(k, state.inner_attrs(), ValueType::for_field(v)),
            )?;
        }

        Ok(())
    }
}

impl<T> ProcessValue for Box<T>
where
    T: ProcessValue,
{
    #[inline]
    fn value_type(&self) -> EnumSet<ValueType> {
        (**self).value_type()
    }

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
        ProcessValue::process_value(self.as_mut(), meta, processor, state)
    }
}

macro_rules! process_tuple {
    ($($name: ident),+) => {
        impl< $( $name: ProcessValue ),* > ProcessValue for ( $( Annotated<$name>, )* ) {
            #[inline]
            fn value_type(&self) -> EnumSet<ValueType> { EnumSet::only(ValueType::Array) }

            #[inline]
            #[allow(non_snake_case, unused_assignments)]
            fn process_child_values<P>(&mut self, processor: &mut P, state: &ProcessingState<'_>)
                -> ProcessingResult
            where
                P: Processor,
            {
                let ($(ref mut $name,)*) = *self;
                let mut index = 0;

                $(
                    process_value($name, processor, &state.enter_index(index, state.inner_attrs(), ValueType::for_field($name)))?;
                    index += 1;
                )*

                Ok(())
            }
        }
    };
}

process_tuple!(T1);
process_tuple!(T1, T2);
process_tuple!(T1, T2, T3);
process_tuple!(T1, T2, T3, T4);
process_tuple!(T1, T2, T3, T4, T5);
process_tuple!(T1, T2, T3, T4, T5, T6);
process_tuple!(T1, T2, T3, T4, T5, T6, T7);
process_tuple!(T1, T2, T3, T4, T5, T6, T7, T8);
process_tuple!(T1, T2, T3, T4, T5, T6, T7, T8, T9);
process_tuple!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10);
process_tuple!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11);
process_tuple!(T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12);

impl ProcessValue for Value {
    fn value_type(&self) -> EnumSet<ValueType> {
        match self {
            Value::Bool(v) => v.value_type(),
            Value::I64(v) => v.value_type(),
            Value::U64(v) => v.value_type(),
            Value::F64(v) => v.value_type(),
            Value::String(v) => v.value_type(),
            Value::Array(v) => v.value_type(),
            Value::Object(v) => v.value_type(),
        }
    }

    fn process_value<P>(
        &mut self,
        meta: &mut Meta,
        processor: &mut P,
        state: &ProcessingState<'_>,
    ) -> ProcessingResult
    where
        P: Processor,
    {
        processor.process_value(self, meta, state)?;

        let state = state.enter_nothing(Some(Cow::Borrowed(state.attrs())));
        macro_rules! process_value {
            ($v:ident) => {{
                processor.before_process(Some(&*$v), meta, &state)?;
                $v.process_value(meta, processor, &state)?;
                processor.after_process(Some(&*$v), meta, &state)?;
            }};
        }

        match self {
            Value::Bool(v) => process_value!(v),
            Value::I64(v) => process_value!(v),
            Value::U64(v) => process_value!(v),
            Value::F64(v) => process_value!(v),
            Value::String(v) => process_value!(v),
            Value::Array(v) => process_value!(v),
            Value::Object(v) => process_value!(v),
        }

        Ok(())
    }
}
