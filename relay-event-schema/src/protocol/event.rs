use std::fmt;
use std::str::FromStr;

use relay_common::time;
use relay_protocol::{
    Annotated, Array, Empty, FiniteF64, FromValue, Getter, GetterIter, IntoValue, Object, Val,
    Value,
};
use sentry_release_parser::Release as ParsedRelease;
use uuid::Uuid;

use crate::processor::ProcessValue;
use crate::protocol::{
    AppContext, Breadcrumb, Breakdowns, BrowserContext, ClientSdkInfo, Contexts, Csp, DebugMeta,
    DefaultContext, DeviceContext, EventType, Exception, ExpectCt, ExpectStaple, Fingerprint,
    GpuContext, Hpkp, LenientString, Level, LogEntry, Measurements, Metrics, MonitorContext,
    OsContext, ProfileContext, RelayInfo, Request, ResponseContext, RuntimeContext, Span, SpanId,
    Stacktrace, Tags, TemplateInfo, Thread, Timestamp, TraceContext, TransactionInfo, User, Values,
};

/// Wrapper around a UUID with slightly different formatting.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EventId(pub Uuid);

impl EventId {
    /// Creates a new event id using a UUID v4.
    #[inline]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Creates a new completely zeroed event id.
    pub fn nil() -> Self {
        Self(Uuid::nil())
    }

    /// Tests if the UUID is nil.
    #[inline]
    pub fn is_nil(&self) -> bool {
        self.0.is_nil()
    }
}

impl Default for EventId {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

relay_protocol::derive_string_meta_structure!(EventId, "event id");

impl ProcessValue for EventId {}

impl fmt::Display for EventId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.as_simple())
    }
}

impl FromStr for EventId {
    type Err = <Uuid as FromStr>::Err;

    fn from_str(uuid_str: &str) -> Result<Self, Self::Err> {
        uuid_str.parse().map(EventId)
    }
}

relay_common::impl_str_serde!(EventId, "an event identifier");

impl TryFrom<&SpanId> for EventId {
    type Error = <EventId as FromStr>::Err;

    fn try_from(value: &SpanId) -> Result<Self, Self::Error> {
        // TODO: Represent SpanId as bytes / u64 so we can call `Uuid::from_u64_pair`.
        let s = format!("0000000000000000{value}");
        s.parse()
    }
}

#[derive(Debug, FromValue, IntoValue, ProcessValue, Empty, Clone, PartialEq)]
pub struct ExtraValue(#[metastructure(max_depth = 7, max_bytes = 16_384)] pub Value);

impl<T: Into<Value>> From<T> for ExtraValue {
    fn from(value: T) -> ExtraValue {
        ExtraValue(value.into())
    }
}

/// An event processing error.
#[derive(Clone, Debug, Default, PartialEq, Empty, FromValue, IntoValue, ProcessValue)]
pub struct EventProcessingError {
    /// The error kind.
    #[metastructure(field = "type", required = true)]
    pub ty: Annotated<String>,

    /// Affected key or deep path.
    pub name: Annotated<String>,

    /// The original value causing this error.
    pub value: Annotated<Value>,

    /// Additional data explaining this error.
    #[metastructure(additional_properties, pii = "maybe")]
    pub other: Object<Value>,
}

/// The grouping config that should be used for grouping this event.
///
/// This is currently only supplied as part of normalization and the payload
/// only permits the ID of the algorithm to be set and no parameters yet.
#[derive(Clone, Debug, Default, PartialEq, Empty, FromValue, IntoValue, ProcessValue)]
pub struct GroupingConfig {
    /// The id of the grouping config.
    #[metastructure(max_chars = 128)]
    pub id: Annotated<String>,
    /// The enhancements configuration.
    pub enhancements: Annotated<String>,
}

/// The sentry v7 event structure.
#[derive(Clone, Debug, Default, PartialEq, Empty, FromValue, IntoValue, ProcessValue)]
#[metastructure(process_func = "process_event", value_type = "Event")]
pub struct Event {
    /// Unique identifier of this event.
    ///
    /// Hexadecimal string representing a uuid4 value. The length is exactly 32 characters. Dashes
    /// are not allowed. Has to be lowercase.
    ///
    /// Even though this field is backfilled on the server with a new uuid4, it is strongly
    /// recommended to generate that uuid4 clientside. There are some features like user feedback
    /// which are easier to implement that way, and debugging in case events get lost in your
    /// Sentry installation is also easier.
    ///
    /// Example:
    ///
    /// ```json
    /// {
    ///   "event_id": "fc6d8c0c43fc4630ad850ee518f1b9d0"
    /// }
    /// ```
    #[metastructure(field = "event_id")]
    pub id: Annotated<EventId>,

    /// Severity level of the event. Defaults to `error`.
    ///
    /// Example:
    ///
    /// ```json
    /// {"level": "warning"}
    /// ```
    pub level: Annotated<Level>,

    /// Version
    pub version: Annotated<String>,

    /// Type of the event. Defaults to `default`.
    ///
    /// The event type determines how Sentry handles the event and has an impact on processing, rate
    /// limiting, and quotas. There are three fundamental classes of event types:
    ///
    ///  - **Error monitoring events**: Processed and grouped into unique issues based on their
    ///    exception stack traces and error messages.
    ///  - **Security events**: Derived from Browser security violation reports and grouped into
    ///    unique issues based on the endpoint and violation. SDKs do not send such events.
    ///  - **Transaction events** (`transaction`): Contain operation spans and collected into traces
    ///    for performance monitoring.
    ///
    /// Transactions must explicitly specify the `"transaction"` event type. In all other cases,
    /// Sentry infers the appropriate event type from the payload and overrides the stated type.
    /// SDKs should not send an event type other than for transactions.
    ///
    /// Example:
    ///
    /// ```json
    /// {
    ///   "type": "transaction",
    ///   "spans": []
    /// }
    /// ```
    #[metastructure(field = "type")]
    pub ty: Annotated<EventType>,

    /// Manual fingerprint override.
    ///
    /// A list of strings used to dictate how this event is supposed to be grouped with other
    /// events into issues. For more information about overriding grouping see [Customize Grouping
    /// with Fingerprints](https://docs.sentry.io/data-management/event-grouping/).
    ///
    /// ```json
    /// {
    ///     "fingerprint": ["myrpc", "POST", "/foo.bar"]
    /// }
    #[metastructure(skip_serialization = "empty")]
    pub fingerprint: Annotated<Fingerprint>,

    /// Custom culprit of the event.
    ///
    /// This field is deprecated and shall not be set by client SDKs.
    #[metastructure(max_chars = 200, pii = "maybe")]
    pub culprit: Annotated<String>,

    /// Transaction name of the event.
    ///
    /// For example, in a web app, this might be the route name (`"/users/<username>/"` or
    /// `UserView`), in a task queue it might be the function + module name.
    #[metastructure(max_chars = 200, trim_whitespace = true)]
    pub transaction: Annotated<String>,

    /// Additional information about the name of the transaction.
    #[metastructure(skip_serialization = "null")]
    pub transaction_info: Annotated<TransactionInfo>,

    /// Time since the start of the transaction until the error occurred.
    pub time_spent: Annotated<u64>,

    /// Custom parameterized message for this event.
    #[metastructure(legacy_alias = "sentry.interfaces.Message", legacy_alias = "message")]
    #[metastructure(skip_serialization = "empty")]
    pub logentry: Annotated<LogEntry>,

    /// Logger that created the event.
    #[metastructure(
        max_chars = 64, // DB-imposed limit
        deny_chars = "\r\n",
    )]
    pub logger: Annotated<String>,

    /// Name and versions of all installed modules/packages/dependencies in the current
    /// environment/application.
    ///
    /// ```json
    /// { "django": "3.0.0", "celery": "4.2.1" }
    /// ```
    ///
    /// In Python this is a list of installed packages as reported by `pkg_resources` together with
    /// their reported version string.
    ///
    /// This is primarily used for suggesting to enable certain SDK integrations from within the UI
    /// and for making informed decisions on which frameworks to support in future development
    /// efforts.
    #[metastructure(skip_serialization = "empty_deep", max_depth = 7, max_bytes = 8192)]
    pub modules: Annotated<Object<String>>,

    /// Platform identifier of this event (defaults to "other").
    ///
    /// A string representing the platform the SDK is submitting from. This will be used by the
    /// Sentry interface to customize various components in the interface, but also to enter or
    /// skip stacktrace processing.
    ///
    /// Acceptable values are: `as3`, `c`, `cfml`, `cocoa`, `csharp`, `elixir`, `haskell`, `go`,
    /// `groovy`, `java`, `javascript`, `native`, `node`, `objc`, `other`, `perl`, `php`, `python`,
    /// `ruby`
    pub platform: Annotated<String>,

    /// Timestamp when the event was created.
    ///
    /// Indicates when the event was created in the Sentry SDK. The format is either a string as
    /// defined in [RFC 3339](https://tools.ietf.org/html/rfc3339) or a numeric (integer or float)
    /// value representing the number of seconds that have elapsed since the [Unix
    /// epoch](https://en.wikipedia.org/wiki/Unix_time).
    ///
    /// Timezone is assumed to be UTC if missing.
    ///
    /// Sub-microsecond precision is not preserved with numeric values due to precision
    /// limitations with floats (at least in our systems). With that caveat in mind, just send
    /// whatever is easiest to produce.
    ///
    /// All timestamps in the event protocol are formatted this way.
    ///
    /// # Example
    ///
    /// All of these are the same date:
    ///
    /// ```json
    /// { "timestamp": "2011-05-02T17:41:36Z" }
    /// { "timestamp": "2011-05-02T17:41:36" }
    /// { "timestamp": "2011-05-02T17:41:36.000" }
    /// { "timestamp": 1304358096.0 }
    /// ```
    pub timestamp: Annotated<Timestamp>,

    /// Timestamp when the event has started (relevant for event type = "transaction")
    #[metastructure(omit_from_schema)] // we only document error events for now
    pub start_timestamp: Annotated<Timestamp>,

    /// Timestamp when the event has been received by Sentry.
    pub received: Annotated<Timestamp>,

    /// Server or device name the event was generated on.
    ///
    /// This is supposed to be a hostname.
    #[metastructure(pii = "true", max_chars = 256, max_chars_allowance = 20)]
    pub server_name: Annotated<String>,

    /// The release version of the application.
    ///
    /// **Release versions must be unique across all projects in your organization.** This value
    /// can be the git SHA for the given project, or a product identifier with a semantic version.
    #[metastructure(
        max_chars = 200,  // release ends in tag
        // release allowed chars are validated in the sentry-release-parser crate!
        required = false,
        trim_whitespace = true,
        nonempty = true,
        skip_serialization = "empty"
    )]
    pub release: Annotated<LenientString>,

    /// Program's distribution identifier.
    ///
    /// The distribution of the application.
    ///
    /// Distributions are used to disambiguate build or deployment variants of the same release of
    /// an application. For example, the dist can be the build number of an XCode build or the
    /// version code of an Android build.
    #[metastructure(
        allow_chars = "a-zA-Z0-9_.-",
        trim_whitespace = true,
        required = false,
        nonempty = true
    )]
    pub dist: Annotated<String>,

    /// The environment name, such as `production` or `staging`.
    ///
    /// ```json
    /// { "environment": "production" }
    /// ```
    #[metastructure(
        max_chars = 64,
        // environment allowed chars are validated in the sentry-release-parser crate!
        nonempty = true,
        required = false,
        trim_whitespace = true
    )]
    pub environment: Annotated<String>,

    /// Deprecated in favor of tags.
    #[metastructure(max_chars = 256, max_chars_allowance = 20)]
    #[metastructure(omit_from_schema)] // deprecated
    pub site: Annotated<String>,

    /// Information about the user who triggered this event.
    #[metastructure(legacy_alias = "sentry.interfaces.User")]
    #[metastructure(skip_serialization = "empty")]
    pub user: Annotated<User>,

    /// Information about a web request that occurred during the event.
    #[metastructure(legacy_alias = "sentry.interfaces.Http")]
    #[metastructure(skip_serialization = "empty")]
    pub request: Annotated<Request>,

    /// Contexts describing the environment (e.g. device, os or browser).
    #[metastructure(legacy_alias = "sentry.interfaces.Contexts")]
    pub contexts: Annotated<Contexts>,

    /// List of breadcrumbs recorded before this event.
    #[metastructure(legacy_alias = "sentry.interfaces.Breadcrumbs")]
    #[metastructure(skip_serialization = "empty")]
    pub breadcrumbs: Annotated<Values<Breadcrumb>>,

    /// One or multiple chained (nested) exceptions.
    #[metastructure(legacy_alias = "sentry.interfaces.Exception")]
    #[metastructure(field = "exception")]
    #[metastructure(skip_serialization = "empty")]
    pub exceptions: Annotated<Values<Exception>>,

    /// Event stacktrace.
    ///
    /// DEPRECATED: Prefer `threads` or `exception` depending on which is more appropriate.
    #[metastructure(skip_serialization = "empty")]
    #[metastructure(legacy_alias = "sentry.interfaces.Stacktrace")]
    pub stacktrace: Annotated<Stacktrace>,

    /// Simplified template error location information.
    /// DEPRECATED: Non-Raven clients are not supposed to send this anymore, but rather just report
    /// synthetic frames.
    #[metastructure(legacy_alias = "sentry.interfaces.Template")]
    #[metastructure(omit_from_schema)]
    pub template: Annotated<TemplateInfo>,

    /// Threads that were active when the event occurred.
    #[metastructure(skip_serialization = "empty")]
    pub threads: Annotated<Values<Thread>>,

    /// Custom tags for this event.
    ///
    /// A map or list of tags for this event. Each tag must be less than 200 characters.
    #[metastructure(skip_serialization = "empty", pii = "maybe")]
    pub tags: Annotated<Tags>,

    /// Arbitrary extra information set by the user.
    ///
    /// ```json
    /// {
    ///     "extra": {
    ///         "my_key": 1,
    ///         "some_other_value": "foo bar"
    ///     }
    /// }```
    #[metastructure(max_depth = 7, max_bytes = 262_144)]
    #[metastructure(pii = "true", skip_serialization = "empty")]
    pub extra: Annotated<Object<ExtraValue>>,

    /// Meta data for event processing and debugging.
    #[metastructure(skip_serialization = "empty")]
    pub debug_meta: Annotated<DebugMeta>,

    /// Information about the Sentry SDK that generated this event.
    #[metastructure(field = "sdk")]
    #[metastructure(skip_serialization = "empty")]
    pub client_sdk: Annotated<ClientSdkInfo>,

    /// Information about the Relays that processed this event during ingest.
    #[metastructure(max_depth = 5, max_bytes = 2048)]
    #[metastructure(skip_serialization = "empty", omit_from_schema)]
    pub ingest_path: Annotated<Array<RelayInfo>>,

    /// Errors encountered during processing. Intended to be phased out in favor of
    /// annotation/metadata system.
    #[metastructure(skip_serialization = "empty_deep")]
    pub errors: Annotated<Array<EventProcessingError>>,

    /// Project key which sent this event.
    #[metastructure(omit_from_schema)] // not part of external schema
    pub key_id: Annotated<String>,

    /// Project which sent this event.
    #[metastructure(omit_from_schema)] // not part of external schema
    pub project: Annotated<u64>,

    /// The grouping configuration for this event.
    #[metastructure(omit_from_schema)] // not part of external schema
    pub grouping_config: Annotated<Object<Value>>,

    /// Legacy checksum used for grouping before fingerprint hashes.
    #[metastructure(max_chars = 128)]
    #[metastructure(omit_from_schema)] // deprecated
    pub checksum: Annotated<String>,

    /// CSP (security) reports.
    #[metastructure(legacy_alias = "sentry.interfaces.Csp")]
    #[metastructure(omit_from_schema)] // we only document error events for now
    pub csp: Annotated<Csp>,

    /// HPKP (security) reports.
    #[metastructure(pii = "true", legacy_alias = "sentry.interfaces.Hpkp")]
    #[metastructure(omit_from_schema)] // we only document error events for now
    pub hpkp: Annotated<Hpkp>,

    /// ExpectCT (security) reports.
    #[metastructure(pii = "true", legacy_alias = "sentry.interfaces.ExpectCT")]
    #[metastructure(omit_from_schema)] // we only document error events for now
    pub expectct: Annotated<ExpectCt>,

    /// ExpectStaple (security) reports.
    #[metastructure(pii = "true", legacy_alias = "sentry.interfaces.ExpectStaple")]
    #[metastructure(omit_from_schema)] // we only document error events for now
    pub expectstaple: Annotated<ExpectStaple>,

    /// Spans for tracing.
    #[metastructure(max_bytes = 819200)]
    #[metastructure(omit_from_schema)] // we only document error events for now
    pub spans: Annotated<Array<Span>>,

    /// Measurements which holds observed values such as web vitals.
    ///
    /// Measurements are only available on transactions. They contain measurement values of observed
    /// values such as Largest Contentful Paint (LCP).
    #[metastructure(skip_serialization = "empty")]
    #[metastructure(omit_from_schema)] // we only document error events for now
    pub measurements: Annotated<Measurements>,

    /// Breakdowns which holds product-defined values such as span operation breakdowns.
    #[metastructure(skip_serialization = "empty")]
    #[metastructure(omit_from_schema)] // we only document error events for now
    pub breakdowns: Annotated<Breakdowns>,

    /// Information about attempts to scrape a JS source or sourcemap file from the web.
    /// This field is populated by sentry.
    #[metastructure(omit_from_schema)] // not part of external schema
    pub scraping_attempts: Annotated<Value>,

    /// Internal ingestion and processing metrics.
    ///
    /// This value should not be ingested and will be overwritten by the store normalizer.
    #[metastructure(omit_from_schema)]
    pub _metrics: Annotated<Metrics>,

    /// Value of the `DynamicSamplingContext` for this event.
    #[metastructure(omit_from_schema)]
    pub _dsc: Annotated<Value>,

    /// Temporary flag that controls where performance issues are detected.
    ///
    /// When the flag is set to true, this transaction event will be skipped for performance issue
    /// detection in favor of the spans pipeline.
    #[metastructure(skip_serialization = "empty", trim = false)]
    pub _performance_issues_spans: Annotated<bool>,

    /// Additional arbitrary fields for forwards compatibility.
    #[metastructure(additional_properties, pii = "true")]
    pub other: Object<Value>,
}

impl Event {
    /// Returns the value of a tag with the given key.
    ///
    /// If tags are specified in a pair list and the tag is declared multiple times, this function
    /// returns the first match.
    pub fn tag_value(&self, tag_key: &str) -> Option<&str> {
        if let Some(tags) = self.tags.value() {
            tags.get(tag_key)
        } else {
            None
        }
    }

    /// Returns `true` if [`modules`](Self::modules) contains the given module.
    pub fn has_module(&self, module_name: &str) -> bool {
        self.modules
            .value()
            .map(|m| m.contains_key(module_name))
            .unwrap_or(false)
    }

    /// Returns the identifier of the client SDK if available.
    ///
    /// Sentry's own SDKs use a naming schema prefixed with `sentry.`. Defaults to `"unknown"`.
    pub fn sdk_name(&self) -> &str {
        if let Some(client_sdk) = self.client_sdk.value() {
            if let Some(name) = client_sdk.name.as_str() {
                return name;
            }
        }

        "unknown"
    }

    /// Returns the version of the client SDK if available.
    ///
    /// Defaults to `"unknown"`.
    pub fn sdk_version(&self) -> &str {
        if let Some(client_sdk) = self.client_sdk.value() {
            if let Some(version) = client_sdk.version.as_str() {
                return version;
            }
        }

        "unknown"
    }

    /// Returns the raw user agent string.
    ///
    /// Returns `Some` if the event's request interface contains a `user-agent` header. Returns
    /// `None` otherwise.
    pub fn user_agent(&self) -> Option<&str> {
        let headers = self.request.value()?.headers.value()?;

        for item in headers.iter() {
            if let Some((o_k, v)) = item.value() {
                if let Some(k) = o_k.as_str() {
                    if k.eq_ignore_ascii_case("user-agent") {
                        return v.as_str();
                    }
                }
            }
        }

        None
    }

    /// Returns extra data at the specified path.
    ///
    /// The path is evaluated recursively where each path component is joined by a period (`"."`).
    /// Periods in extra keys are not supported.
    pub fn extra_at(&self, path: &str) -> Option<&Value> {
        let mut path = path.split('.');

        // Get the top-level item explicitly, since those have a different type
        let mut value = &self.extra.value()?.get(path.next()?)?.value()?.0;

        // Iterate recursively to fetch nested values
        for key in path {
            if let Value::Object(object) = value {
                value = object.get(key)?.value()?;
            } else {
                return None;
            }
        }

        Some(value)
    }

    /// Returns parsed components of the Release string in [`Self::release`].
    pub fn parse_release(&self) -> Option<ParsedRelease<'_>> {
        sentry_release_parser::Release::parse(self.release.as_str()?).ok()
    }

    /// Returns the numeric measurement value.
    ///
    /// The name is provided without a prefix, for example `"lcp"` loads `event.measurements.lcp`.
    pub fn measurement(&self, name: &str) -> Option<FiniteF64> {
        let annotated = self.measurements.value()?.get(name)?;
        Some(*annotated.value()?.value.value()?)
    }

    /// Returns the numeric breakdown value.
    pub fn breakdown(&self, breakdown: &str, measurement: &str) -> Option<FiniteF64> {
        let breakdown = self.breakdowns.value()?.get(breakdown)?.value()?;
        Some(*breakdown.get(measurement)?.value()?.value.value()?)
    }

    /// Returns a reference to the context if it exists in its default key.
    pub fn context<C: DefaultContext>(&self) -> Option<&C> {
        self.contexts.value()?.get()
    }

    /// Returns a mutable reference to the context if it exists in its default key.
    pub fn context_mut<C: DefaultContext>(&mut self) -> Option<&mut C> {
        self.contexts.value_mut().as_mut()?.get_mut()
    }
}

fn or_none(string: &Annotated<impl AsRef<str>>) -> Option<&str> {
    match string.as_str() {
        None | Some("") => None,
        Some(other) => Some(other),
    }
}

impl Getter for Event {
    fn get_value(&self, path: &str) -> Option<Val<'_>> {
        Some(match path.strip_prefix("event.")? {
            // Simple fields
            "level" => self.level.value()?.name().into(),
            "release" => self.release.as_str()?.into(),
            "dist" => self.dist.as_str()?.into(),
            "environment" => self.environment.as_str()?.into(),
            "transaction" => self.transaction.as_str()?.into(),
            "logger" => self.logger.as_str()?.into(),
            "platform" => self.platform.as_str().unwrap_or("other").into(),

            // Fields in top level structures (called "interfaces" in Sentry)
            "logentry.formatted" => self.logentry.value()?.formatted.value()?.as_ref().into(),
            "logentry.message" => self.logentry.value()?.message.value()?.as_ref().into(),
            "user.email" => or_none(&self.user.value()?.email)?.into(),
            "user.id" => or_none(&self.user.value()?.id)?.into(),
            "user.ip_address" => self.user.value()?.ip_address.as_str()?.into(),
            "user.name" => self.user.value()?.name.as_str()?.into(),
            "user.segment" => or_none(&self.user.value()?.segment)?.into(),
            "user.geo.city" => self.user.value()?.geo.value()?.city.as_str()?.into(),
            "user.geo.country_code" => self
                .user
                .value()?
                .geo
                .value()?
                .country_code
                .as_str()?
                .into(),
            "user.geo.region" => self.user.value()?.geo.value()?.region.as_str()?.into(),
            "user.geo.subdivision" => self.user.value()?.geo.value()?.subdivision.as_str()?.into(),
            "request.method" => self.request.value()?.method.as_str()?.into(),
            "request.url" => self.request.value()?.url.as_str()?.into(),
            "transaction.source" => self
                .transaction_info
                .value()?
                .source
                .value()?
                .as_str()
                .into(),
            "sdk.name" => self.client_sdk.value()?.name.as_str()?.into(),
            "sdk.version" => self.client_sdk.value()?.version.as_str()?.into(),

            // Computed fields (after normalization).
            "sentry_user" => self.user.value()?.sentry_user.as_str()?.into(),

            // Partial implementation of contexts.
            "contexts.app.in_foreground" => {
                self.context::<AppContext>()?.in_foreground.value()?.into()
            }
            "contexts.app.device_app_hash" => self
                .context::<AppContext>()?
                .device_app_hash
                .as_str()?
                .into(),
            "contexts.device.arch" => self.context::<DeviceContext>()?.arch.as_str()?.into(),
            "contexts.device.battery_level" => self
                .context::<DeviceContext>()?
                .battery_level
                .value()?
                .into(),
            "contexts.device.brand" => self.context::<DeviceContext>()?.brand.as_str()?.into(),
            "contexts.device.charging" => self.context::<DeviceContext>()?.charging.value()?.into(),
            "contexts.device.family" => self.context::<DeviceContext>()?.family.as_str()?.into(),
            "contexts.device.model" => self.context::<DeviceContext>()?.model.as_str()?.into(),
            "contexts.device.locale" => self.context::<DeviceContext>()?.locale.as_str()?.into(),
            "contexts.device.online" => self.context::<DeviceContext>()?.online.value()?.into(),
            "contexts.device.orientation" => self
                .context::<DeviceContext>()?
                .orientation
                .as_str()?
                .into(),
            "contexts.device.name" => self.context::<DeviceContext>()?.name.as_str()?.into(),
            "contexts.device.screen_density" => self
                .context::<DeviceContext>()?
                .screen_density
                .value()?
                .into(),
            "contexts.device.screen_dpi" => {
                self.context::<DeviceContext>()?.screen_dpi.value()?.into()
            }
            "contexts.device.screen_width_pixels" => self
                .context::<DeviceContext>()?
                .screen_width_pixels
                .value()?
                .into(),
            "contexts.device.screen_height_pixels" => self
                .context::<DeviceContext>()?
                .screen_height_pixels
                .value()?
                .into(),
            "contexts.device.simulator" => {
                self.context::<DeviceContext>()?.simulator.value()?.into()
            }
            "contexts.gpu.vendor_name" => {
                self.context::<GpuContext>()?.vendor_name.as_str()?.into()
            }
            "contexts.gpu.name" => self.context::<GpuContext>()?.name.as_str()?.into(),
            "contexts.monitor.id" => self.context::<MonitorContext>()?.get("id")?.value()?.into(),
            "contexts.monitor.slug" => self
                .context::<MonitorContext>()?
                .get("slug")?
                .value()?
                .into(),
            "contexts.os" => self.context::<OsContext>()?.os.as_str()?.into(),
            "contexts.os.build" => self.context::<OsContext>()?.build.as_str()?.into(),
            "contexts.os.kernel_version" => {
                self.context::<OsContext>()?.kernel_version.as_str()?.into()
            }
            "contexts.os.name" => self.context::<OsContext>()?.name.as_str()?.into(),
            "contexts.os.version" => self.context::<OsContext>()?.version.as_str()?.into(),
            "contexts.os.rooted" => self.context::<OsContext>()?.rooted.value()?.into(),
            "contexts.browser" => self.context::<BrowserContext>()?.browser.as_str()?.into(),
            "contexts.browser.name" => self.context::<BrowserContext>()?.name.as_str()?.into(),
            "contexts.browser.version" => {
                self.context::<BrowserContext>()?.version.as_str()?.into()
            }
            "contexts.profile.profile_id" => {
                (&self.context::<ProfileContext>()?.profile_id.value()?.0).into()
            }
            "contexts.device.uuid" => self.context::<DeviceContext>()?.uuid.value()?.into(),
            "contexts.trace.status" => self
                .context::<TraceContext>()?
                .status
                .value()?
                .as_str()
                .into(),
            "contexts.trace.op" => self.context::<TraceContext>()?.op.as_str()?.into(),
            "contexts.response.status_code" => self
                .context::<ResponseContext>()?
                .status_code
                .value()?
                .into(),
            "contexts.unreal.crash_type" => match self.contexts.value()?.get_key("unreal")? {
                super::Context::Other(context) => context.get("crash_type")?.value()?.into(),
                _ => return None,
            },
            "contexts.runtime" => self.context::<RuntimeContext>()?.runtime.as_str()?.into(),
            "contexts.runtime.name" => self.context::<RuntimeContext>()?.name.as_str()?.into(),

            // Computed fields (see Discover)
            "duration" => {
                let start = self.start_timestamp.value()?;
                let end = self.timestamp.value()?;
                if start <= end && self.ty.value() == Some(&EventType::Transaction) {
                    time::chrono_to_positive_millis(*end - *start).into()
                } else {
                    return None;
                }
            }

            // Dynamic access to certain data bags
            path => {
                if let Some(rest) = path.strip_prefix("release.") {
                    let release = self.parse_release()?;
                    match rest {
                        "build" => release.build_hash()?.into(),
                        "package" => release.package()?.into(),
                        "version.short" => release.version()?.raw_short().into(),
                        _ => return None,
                    }
                } else if let Some(rest) = path.strip_prefix("measurements.") {
                    let name = rest.strip_suffix(".value")?;
                    self.measurement(name)?.into()
                } else if let Some(rest) = path.strip_prefix("breakdowns.") {
                    let (breakdown, measurement) = rest.split_once('.')?;
                    self.breakdown(breakdown, measurement)?.into()
                } else if let Some(rest) = path.strip_prefix("extra.") {
                    self.extra_at(rest)?.into()
                } else if let Some(rest) = path.strip_prefix("tags.") {
                    self.tags.value()?.get(rest)?.into()
                } else if let Some(rest) = path.strip_prefix("request.headers.") {
                    self.request
                        .value()?
                        .headers
                        .value()?
                        .get_header(rest)?
                        .into()
                } else {
                    return None;
                }
            }
        })
    }

    fn get_iter(&self, path: &str) -> Option<GetterIter<'_>> {
        Some(match path.strip_prefix("event.")? {
            "exception.values" => {
                GetterIter::new_annotated(self.exceptions.value()?.values.value()?)
            }
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use relay_protocol::{ErrorKind, HexId, Map, Meta};
    use similar_asserts::assert_eq;
    use std::collections::BTreeMap;
    use uuid::uuid;

    use super::*;
    use crate::protocol::{
        Headers, IpAddr, JsonLenientString, PairList, TagEntry, TransactionSource,
    };

    #[test]
    fn test_event_roundtrip() {
        // NOTE: Interfaces will be tested separately.
        let json = r#"{
  "event_id": "52df9022835246eeb317dbd739ccd059",
  "level": "debug",
  "fingerprint": [
    "myprint"
  ],
  "culprit": "myculprit",
  "transaction": "mytransaction",
  "logentry": {
    "formatted": "mymessage"
  },
  "logger": "mylogger",
  "modules": {
    "mymodule": "1.0.0"
  },
  "platform": "myplatform",
  "timestamp": 946684800.0,
  "server_name": "myhost",
  "release": "myrelease",
  "dist": "mydist",
  "environment": "myenv",
  "tags": [
    [
      "tag",
      "value"
    ]
  ],
  "extra": {
    "extra": "value"
  },
  "other": "value",
  "_meta": {
    "event_id": {
      "": {
        "err": [
          "invalid_data"
        ]
      }
    }
  }
}"#;

        let event = Annotated::new(Event {
            id: Annotated(
                Some("52df9022-8352-46ee-b317-dbd739ccd059".parse().unwrap()),
                Meta::from_error(ErrorKind::InvalidData),
            ),
            level: Annotated::new(Level::Debug),
            fingerprint: Annotated::new(vec!["myprint".to_owned()].into()),
            culprit: Annotated::new("myculprit".to_owned()),
            transaction: Annotated::new("mytransaction".to_owned()),
            logentry: Annotated::new(LogEntry {
                formatted: Annotated::new("mymessage".to_owned().into()),
                ..Default::default()
            }),
            logger: Annotated::new("mylogger".to_owned()),
            modules: {
                let mut map = Map::new();
                map.insert("mymodule".to_owned(), Annotated::new("1.0.0".to_owned()));
                Annotated::new(map)
            },
            platform: Annotated::new("myplatform".to_owned()),
            timestamp: Annotated::new(Utc.with_ymd_and_hms(2000, 1, 1, 0, 0, 0).unwrap().into()),
            server_name: Annotated::new("myhost".to_owned()),
            release: Annotated::new("myrelease".to_owned().into()),
            dist: Annotated::new("mydist".to_owned()),
            environment: Annotated::new("myenv".to_owned()),
            tags: {
                let items = vec![Annotated::new(TagEntry(
                    Annotated::new("tag".to_owned()),
                    Annotated::new("value".to_owned()),
                ))];
                Annotated::new(Tags(items.into()))
            },
            extra: {
                let mut map = Map::new();
                map.insert(
                    "extra".to_owned(),
                    Annotated::new(ExtraValue(Value::String("value".to_owned()))),
                );
                Annotated::new(map)
            },
            other: {
                let mut map = Map::new();
                map.insert(
                    "other".to_owned(),
                    Annotated::new(Value::String("value".to_owned())),
                );
                map
            },
            ..Default::default()
        });

        assert_eq!(event, Annotated::from_json(json).unwrap());
        assert_eq!(json, event.to_json_pretty().unwrap());
    }

    #[test]
    fn test_event_default_values() {
        let json = "{}";
        let event = Annotated::new(Event::default());

        assert_eq!(event, Annotated::from_json(json).unwrap());
        assert_eq!(json, event.to_json_pretty().unwrap());
    }

    #[test]
    fn test_event_default_values_with_meta() {
        let json = r#"{
  "event_id": "52df9022835246eeb317dbd739ccd059",
  "fingerprint": [
    "{{ default }}"
  ],
  "platform": "other",
  "_meta": {
    "event_id": {
      "": {
        "err": [
          "invalid_data"
        ]
      }
    },
    "fingerprint": {
      "": {
        "err": [
          "invalid_data"
        ]
      }
    },
    "platform": {
      "": {
        "err": [
          "invalid_data"
        ]
      }
    }
  }
}"#;

        let event = Annotated::new(Event {
            id: Annotated(
                Some("52df9022-8352-46ee-b317-dbd739ccd059".parse().unwrap()),
                Meta::from_error(ErrorKind::InvalidData),
            ),
            fingerprint: Annotated(
                Some(vec!["{{ default }}".to_owned()].into()),
                Meta::from_error(ErrorKind::InvalidData),
            ),
            platform: Annotated(
                Some("other".to_owned()),
                Meta::from_error(ErrorKind::InvalidData),
            ),
            ..Default::default()
        });

        assert_eq!(event, Annotated::<Event>::from_json(json).unwrap());
        assert_eq!(json, event.to_json_pretty().unwrap());
    }

    #[test]
    fn test_event_type() {
        assert_eq!(
            EventType::Default,
            *Annotated::<EventType>::from_json("\"default\"")
                .unwrap()
                .value()
                .unwrap()
        );
    }

    #[test]
    fn test_fingerprint_empty_string() {
        let json = r#"{"fingerprint":[""]}"#;
        let event = Annotated::new(Event {
            fingerprint: Annotated::new(vec!["".to_owned()].into()),
            ..Default::default()
        });

        assert_eq!(json, event.to_json().unwrap());
        assert_eq!(event, Annotated::from_json(json).unwrap());
    }

    #[test]
    fn test_fingerprint_null_values() {
        let input = r#"{"fingerprint":[null]}"#;
        let output = r#"{}"#;
        let event = Annotated::new(Event {
            fingerprint: Annotated::new(vec![].into()),
            ..Default::default()
        });

        assert_eq!(event, Annotated::from_json(input).unwrap());
        assert_eq!(output, event.to_json().unwrap());
    }

    #[test]
    fn test_empty_threads() {
        let input = r#"{"threads": {}}"#;
        let output = r#"{}"#;

        let event = Annotated::new(Event::default());

        assert_eq!(event, Annotated::from_json(input).unwrap());
        assert_eq!(output, event.to_json().unwrap());
    }

    #[test]
    fn test_lenient_release() {
        let input = r#"{"release":42}"#;
        let output = r#"{"release":"42"}"#;
        let event = Annotated::new(Event {
            release: Annotated::new("42".to_owned().into()),
            ..Default::default()
        });

        assert_eq!(event, Annotated::from_json(input).unwrap());
        assert_eq!(output, event.to_json().unwrap());
    }

    #[test]
    fn test_extra_at() {
        let json = serde_json::json!({
            "extra": {
                "a": "string1",
                "b": 42,
                "c": {
                    "d": "string2",
                    "e": null,
                },
            },
        });

        let event = Event::from_value(json.into());
        let event = event.value().unwrap();

        assert_eq!(
            Some(&Value::String("string1".to_owned())),
            event.extra_at("a")
        );
        assert_eq!(Some(&Value::I64(42)), event.extra_at("b"));
        assert!(matches!(event.extra_at("c"), Some(&Value::Object(_))));
        assert_eq!(None, event.extra_at("d"));
        assert_eq!(
            Some(&Value::String("string2".to_owned())),
            event.extra_at("c.d")
        );
        assert_eq!(None, event.extra_at("c.e"));
        assert_eq!(None, event.extra_at("c.f"));
    }

    #[test]
    fn test_scrape_attempts() {
        let json = serde_json::json!({
            "scraping_attempts": [
                {"status": "not_attempted", "url": "http://example.com/embedded.js"},
                {"status": "not_attempted", "url": "http://example.com/embedded.js.map"},
            ]
        });

        let event = Event::from_value(json.into());
        assert!(!event.value().unwrap().scraping_attempts.meta().has_errors());
    }

    #[test]
    fn test_field_value_provider_event_filled() {
        let event = Event {
            level: Annotated::new(Level::Info),
            release: Annotated::new(LenientString("1.1.1".to_owned())),
            environment: Annotated::new("prod".to_owned()),
            user: Annotated::new(User {
                ip_address: Annotated::new(IpAddr("127.0.0.1".to_owned())),
                id: Annotated::new(LenientString("user-id".into())),
                segment: Annotated::new("user-seg".into()),
                sentry_user: Annotated::new("id:user-id".into()),
                ..Default::default()
            }),
            client_sdk: Annotated::new(ClientSdkInfo {
                name: Annotated::new("sentry-javascript".into()),
                version: Annotated::new("1.87.0".into()),
                ..Default::default()
            }),
            exceptions: Annotated::new(Values {
                values: Annotated::new(vec![Annotated::new(Exception {
                    value: Annotated::new(JsonLenientString::from(
                        "canvas.contentDocument".to_owned(),
                    )),
                    ..Default::default()
                })]),
                ..Default::default()
            }),
            logentry: Annotated::new(LogEntry {
                formatted: Annotated::new("formatted".to_owned().into()),
                message: Annotated::new("message".to_owned().into()),
                ..Default::default()
            }),
            request: Annotated::new(Request {
                headers: Annotated::new(Headers(PairList(vec![Annotated::new((
                    Annotated::new("user-agent".into()),
                    Annotated::new("Slurp".into()),
                ))]))),
                url: Annotated::new("https://sentry.io".into()),
                ..Default::default()
            }),
            transaction: Annotated::new("some-transaction".into()),
            transaction_info: Annotated::new(TransactionInfo {
                source: Annotated::new(TransactionSource::Route),
                ..Default::default()
            }),
            tags: {
                let items = vec![Annotated::new(TagEntry(
                    Annotated::new("custom".to_owned()),
                    Annotated::new("custom-value".to_owned()),
                ))];
                Annotated::new(Tags(items.into()))
            },
            contexts: Annotated::new({
                let mut contexts = Contexts::new();
                contexts.add(DeviceContext {
                    name: Annotated::new("iphone".to_owned()),
                    family: Annotated::new("iphone-fam".to_owned()),
                    model: Annotated::new("iphone7,3".to_owned()),
                    screen_dpi: Annotated::new(560),
                    screen_width_pixels: Annotated::new(1920),
                    screen_height_pixels: Annotated::new(1080),
                    locale: Annotated::new("US".into()),
                    uuid: Annotated::new(uuid!("abadcade-feed-dead-beef-baddadfeeded")),
                    charging: Annotated::new(true),
                    ..DeviceContext::default()
                });
                contexts.add(OsContext {
                    name: Annotated::new("iOS".to_owned()),
                    version: Annotated::new("11.4.2".to_owned()),
                    kernel_version: Annotated::new("17.4.0".to_owned()),
                    ..OsContext::default()
                });
                contexts.add(ProfileContext {
                    profile_id: Annotated::new(EventId(uuid!(
                        "abadcade-feed-dead-beef-8addadfeedaa"
                    ))),
                    ..ProfileContext::default()
                });
                let mut monitor_context_fields = BTreeMap::new();
                monitor_context_fields.insert(
                    "id".to_owned(),
                    Annotated::new(Value::String("123".to_owned())),
                );
                monitor_context_fields.insert(
                    "slug".to_owned(),
                    Annotated::new(Value::String("my_monitor".to_owned())),
                );
                contexts.add(MonitorContext(monitor_context_fields));
                contexts
            }),
            ..Default::default()
        };

        assert_eq!(Some(Val::String("info")), event.get_value("event.level"));

        assert_eq!(Some(Val::String("1.1.1")), event.get_value("event.release"));
        assert_eq!(
            Some(Val::String("prod")),
            event.get_value("event.environment")
        );
        assert_eq!(
            Some(Val::String("user-id")),
            event.get_value("event.user.id")
        );
        assert_eq!(
            Some(Val::String("id:user-id")),
            event.get_value("event.sentry_user")
        );
        assert_eq!(
            Some(Val::String("user-seg")),
            event.get_value("event.user.segment")
        );
        assert_eq!(
            Some(Val::String("some-transaction")),
            event.get_value("event.transaction")
        );
        assert_eq!(
            Some(Val::String("iphone")),
            event.get_value("event.contexts.device.name")
        );
        assert_eq!(
            Some(Val::String("iphone-fam")),
            event.get_value("event.contexts.device.family")
        );
        assert_eq!(
            Some(Val::String("iOS")),
            event.get_value("event.contexts.os.name")
        );
        assert_eq!(
            Some(Val::String("11.4.2")),
            event.get_value("event.contexts.os.version")
        );
        assert_eq!(
            Some(Val::String("custom-value")),
            event.get_value("event.tags.custom")
        );
        assert_eq!(None, event.get_value("event.tags.doesntexist"));
        assert_eq!(
            Some(Val::String("sentry-javascript")),
            event.get_value("event.sdk.name")
        );
        assert_eq!(
            Some(Val::String("1.87.0")),
            event.get_value("event.sdk.version")
        );
        assert_eq!(
            Some(Val::String("17.4.0")),
            event.get_value("event.contexts.os.kernel_version")
        );
        assert_eq!(
            Some(Val::I64(560)),
            event.get_value("event.contexts.device.screen_dpi")
        );
        assert_eq!(
            Some(Val::Bool(true)),
            event.get_value("event.contexts.device.charging")
        );
        assert_eq!(
            Some(Val::U64(1920)),
            event.get_value("event.contexts.device.screen_width_pixels")
        );
        assert_eq!(
            Some(Val::U64(1080)),
            event.get_value("event.contexts.device.screen_height_pixels")
        );
        assert_eq!(
            Some(Val::String("US")),
            event.get_value("event.contexts.device.locale")
        );
        assert_eq!(
            Some(Val::HexId(HexId(
                uuid!("abadcade-feed-dead-beef-baddadfeeded").as_bytes()
            ))),
            event.get_value("event.contexts.device.uuid")
        );
        assert_eq!(
            Some(Val::String("https://sentry.io")),
            event.get_value("event.request.url")
        );
        assert_eq!(
            Some(Val::HexId(HexId(
                uuid!("abadcade-feed-dead-beef-8addadfeedaa").as_bytes()
            ))),
            event.get_value("event.contexts.profile.profile_id")
        );
        assert_eq!(
            Some(Val::String("route")),
            event.get_value("event.transaction.source")
        );

        let mut exceptions = event.get_iter("event.exception.values").unwrap();
        let exception = exceptions.next().unwrap();
        assert_eq!(
            Some(Val::String("canvas.contentDocument")),
            exception.get_value("value")
        );
        assert!(exceptions.next().is_none());

        assert_eq!(
            Some(Val::String("formatted")),
            event.get_value("event.logentry.formatted")
        );
        assert_eq!(
            Some(Val::String("message")),
            event.get_value("event.logentry.message")
        );
        assert_eq!(
            Some(Val::String("123")),
            event.get_value("event.contexts.monitor.id")
        );
        assert_eq!(
            Some(Val::String("my_monitor")),
            event.get_value("event.contexts.monitor.slug")
        );
    }

    #[test]
    fn test_field_value_provider_event_empty() {
        let event = Event::default();

        assert_eq!(None, event.get_value("event.release"));
        assert_eq!(None, event.get_value("event.environment"));
        assert_eq!(None, event.get_value("event.user.id"));
        assert_eq!(None, event.get_value("event.user.segment"));

        // now try with an empty user
        let event = Event {
            user: Annotated::new(User {
                ..Default::default()
            }),
            ..Default::default()
        };

        assert_eq!(None, event.get_value("event.user.id"));
        assert_eq!(None, event.get_value("event.user.segment"));
        assert_eq!(None, event.get_value("event.transaction"));
    }
}
