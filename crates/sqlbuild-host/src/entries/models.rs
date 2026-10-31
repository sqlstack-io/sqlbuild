//! Host entries: the staying-Python names native code reaches through `sqlbuild._native_host`.

/// What a host entry resolves to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostKind {
    Function,
    Class,
    Exception,
    Value,
}

impl HostKind {
    /// The kind as the import test and `_native.HOST_ENTRIES` spell it.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Class => "class",
            Self::Exception => "exception",
            Self::Value => "value",
        }
    }
}

/// Which boundary a host entry crosses; also its group in `HOST_ENTRIES`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostEdge {
    Adapter,
    UserCode,
    Executor,
    ThirdParty,
    Facade,
}

impl HostEdge {
    /// The edge as `_native.HOST_ENTRIES` spells it.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Adapter => "adapter",
            Self::UserCode => "user_code",
            Self::Executor => "executor",
            Self::ThirdParty => "third_party",
            Self::Facade => "facade",
        }
    }
}

/// One host entry: its `sqlbuild._native_host` name, kind and edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostEntry {
    pub name: &'static str,
    pub kind: HostKind,
    pub edge: HostEdge,
}

/// A host entry by edge and position, from any edge's entry enum.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostId {
    pub edge: HostEdge,
    pub index: usize,
}

/// One adapter entry of `sqlbuild._native_host`, indexing its `ENTRIES`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdapterHost {
    BaseAdapter,
    AdapterUserError,
    ColumnInfo,
    ExpressionInferenceProfile,
    RelationInfo,
    RelationLookup,
    RetentionRequest,
    RowDiffSampling,
    RowDiffTolerance,
    RowDiffTolerances,
    TableFreshnessMetadata,
    TableFreshnessRequest,
    BuiltinAdapter,
    FrameworkType,
    LifeCycleEventKind,
    MigrationTransfer,
    RetentionScope,
    TableFreshnessStatus,
    AdapterSupportsSessionWarehouse,
    BuiltinAdapterClasses,
    DiscoverProjectAdapters,
    ResolveAdapter,
    GetColumnsForInspection,
    ListRelationsForInspection,
    ActiveInspectionCatalog,
    InspectionQuerySink,
    OpenInspectionCatalog,
    RecordInspectionQuery,
    BuildRelationLookup,
    ResolveRelationLocationQualifiedName,
    RunBoundedInspections,
    RunRecordedInspectionQuery,
    InspectionQueryRecord,
    StatementMetadataEffect,
    ConditionalResultNullability,
    FirstArgNullability,
    NormalizeRelationType,
    NormalizeType,
    SafeCastNullability,
    ConnectRecallStore,
    RunRecallQuery,
    RunRecallCommand,
    InsertRecallRows,
    CloseRecallStore,
    RelationMetadataCacheScope,
}

impl AdapterHost {
    /// The entry's position in `HOST_ENTRIES`, usable in constant tables.
    pub const fn id(self) -> HostId {
        HostId {
            edge: HostEdge::Adapter,
            index: self as usize,
        }
    }
}

impl From<AdapterHost> for HostId {
    fn from(entry: AdapterHost) -> Self {
        entry.id()
    }
}

/// One user-code entry of `sqlbuild._native_host`, indexing its `ENTRIES`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserCodeHost {
    PythonResourceRows,
    TargetContextWithMacroCallStore,
    InventoryProjectMacros,
    LoadProjectMacros,
    MacroInvocationContext,
    DiscoverCommandOutputSinkFunctions,
    ImportPythonRootModule,
    InstallPythonRootPackage,
    LoadMaterializeFunction,
    LoadProviderModule,
    LoadPythonNodeModule,
    LoadSinkModule,
    LoadOrchestrationModule,
    ModuleFunctions,
    ModuleProviderClasses,
    ParameterAnnotations,
    ProviderUsages,
    RequiresArguments,
    RestorePythonRootModules,
    SourceLine,
    ValidateEventExporterSignature,
    ValidateDiscoveredInputs,
    DiscoverRuntimeExtensions,
    RunWithMacroBridge,
    MacroStoreModulePaths,
    ActiveMacroBridge,
    LoadCustomMaterializations,
    PythonNodeDependencyKey,
    PythonNodeIdentity,
    BuildHookIdentities,
    PythonNodeKind,
    PythonNodeStatus,
    ProviderInputError,
    BuildProviderSession,
    ReadAssetDefinition,
    ReadAuditFactoryDefinition,
    ReadCheckDefinition,
    ReadFactoryDefinition,
    ReadHookDefinition,
    ReadLoaderDefinition,
    ReadTaskDefinition,
    AuditCase,
    RelationRenderer,
    SqlResourceRef,
    PythonCheckSeverity,
    SqlResourceRefKind,
    LoadCustomRules,
    CustomRulePayloads,
    EventExporterInputError,
    EventExporterStateError,
    InvokeLifecycleEventSink,
    RunCommandOutputSinks,
    GetLifecycleEventSinkDefinition,
    InvokeRecallSink,
}

impl UserCodeHost {
    /// The entry's position in `HOST_ENTRIES`, usable in constant tables.
    pub const fn id(self) -> HostId {
        HostId {
            edge: HostEdge::UserCode,
            index: self as usize,
        }
    }
}

impl From<UserCodeHost> for HostId {
    fn from(entry: UserCodeHost) -> Self {
        entry.id()
    }
}

/// One executor entry of `sqlbuild._native_host`, indexing its `ENTRIES`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutorHost {
    ProjectRelation,
    ProjectRelationIndex,
    RelationName,
    CostResourceContext,
    RunCostSummary,
    CostCapability,
    CostStatus,
    BoundSentinel,
    ReadProcessResources,
    SqlPrivateRecordMarker,
    DiagnosticsContext,
    LogDebugEvent,
    LogSql,
    ProcessResourceReporting,
    ProcessResourceUsage,
    ExecutorInputError,
    CurrentAuditResultProjection,
    AuditExecutionResult,
    BuildCallbacks,
    BuildCustomizations,
    BuildExecutionResult,
    BuildInitialState,
    BuildRuntimeParams,
    FunctionExecutionResult,
    SeedExecutionResult,
    BuildStatus,
    BuildDestinationTransientModels,
    BuildDestinationRetentionRequests,
    CopyCloneFingerprints,
    RunPrephaseCloneStream,
    ExecuteClone,
    CloneExecutionInput,
    CloneItemResult,
    CloneSourceEntries,
    CloneStatus,
    QueryDiffArtifactLifecycle,
    FullDiffSizeGuardError,
    ExecuteQueryDiff,
    ExecuteDiff,
    DiffExecutionOptions,
    DiffExecutionResult,
    FullDiffSizeLimits,
    RowDiffSamplingOverride,
    ExecuteJanitorPlan,
    JanitorArchiveCandidate,
    JanitorArchivedRelation,
    JanitorBlockedSchema,
    JanitorDeleteCandidate,
    JanitorDirectStatePruneCandidate,
    JanitorOldNameView,
    JanitorOldNameViewPlanning,
    JanitorPlan,
    JanitorQueryDiffArtifactCandidate,
    JanitorRelationKey,
    JanitorSkippedRelation,
    JanitorSkippedSchema,
    JanitorUsageCheck,
    RunLoadPipeline,
    LoadCallbacks,
    LoadExecutionResult,
    LoadRuntimeParams,
    MigrationRelationForLocation,
    IsCompatibilityView,
    AuditExecutionError,
    WarehouseConnectionError,
    TemporaryTestFunctionError,
    RaiseWarehouseConnectionFailure,
    AuditPipelineCallbacks,
    RunAuditPipelineWithCallbacks,
    RunBuildPipeline,
    RunScenarioCapturePipeline,
    RunScenarioLocalTestPipeline,
    RunScenarioTestPipeline,
    RunSeedPipeline,
    RunTestPipeline,
    SelectScenarioSnapshotCaptureCandidates,
    TestPipelineCallbacks,
    RunItemTimeouts,
    ScenarioTestSettings,
    ExecutePythonChecks,
    ExecuteIngressPythonLoaderNodes,
    RunIngressPythonLoaderNodes,
    CreateReadSidePythonExecutionTracker,
    IngressCallbacks,
    PythonCheckCallbacks,
    PythonCheckExecutionResult,
    PythonNodeExecutionResult,
    PythonNodeRunState,
    PythonNodeRuntime,
    ModelExecutionResult,
    ScenarioCaptureSettings,
    ScenarioLocalReplaySource,
    ScenarioSnapshotCaptureLimits,
    ExecutionStatus,
    SqlTestExecutionResult,
    MicrobatchEventCodec,
    MicrobatchColumns,
    MicrobatchColumnTypes,
    MicrobatchDirectIndexes,
    MicrobatchGenerationWildcard,
    MicrobatchTableName,
    MicrobatchStateError,
    MicrobatchCoverageProjection,
    MicrobatchInterval,
    ProjectedMicrobatchInterval,
    ReplayRequirementProjection,
    ReplayRequirementState,
    TextComputeLogTee,
    CaptureAlreadyExistsError,
    CaptureNotFoundError,
    CaptureStateError,
    ComputeLogMetadataError,
    ComputeLogPathError,
    InvalidCaptureIdError,
    InvalidComputeLogCursorError,
    InvalidComputeLogLimitError,
    CaptureByteCounts,
    CaptureInventory,
    CaptureInventoryItem,
    CaptureMetadata,
    ComputeLogReadChunk,
    FinalCaptureMetadata,
    PruneResult,
    ComputeLogStream,
    ConnectionHooks,
    ExecutionResourceKind,
    ExecutionHistoryStorageError,
    IntegrityConflictError,
    InvalidCursorError,
    InvalidEventError,
    InvalidFilterError,
    InvalidLimitError,
    ProjectionConsistencyError,
    UnsupportedSchemaVersionError,
    EventPage,
    RunPage,
    RunRecord,
    StoredEvent,
    RunStatus,
    ExecutionDurationLimitError,
    ExecutionDeadlineScope,
    AuditEvidenceScope,
    CancelRunningStatements,
    EventDispatcher,
    OperationLifecycle,
    StatementLifecycle,
    ObservabilityValidationError,
    CreateLifecycleEvent,
    CurrentExecutionIdentity,
    DiagnosticLogToJson,
    DispatcherScope,
    InvocationExternalContextScope,
    InvocationScope,
    LifecycleEventFromJson,
    LifecycleEventToJson,
    RunScope,
    DiagnosticLog,
    LifecycleEvent,
    OperationAttributes,
    CommandOutputValidationError,
    OutputCaptureInputError,
    ConfiguredOutputCaptureScope,
    BoundCommandOutputSink,
    CommandOutputCaptureSummary,
    CommandOutputRecord,
    OutputCaptureContext,
    CommandOutputStream,
    OutputRecordPriority,
    SqlValueRenderingError,
    CreateReviewSchema,
    DropReviewSchema,
    LeftoverReviewSchemas,
}

impl ExecutorHost {
    /// The entry's position in `HOST_ENTRIES`, usable in constant tables.
    pub const fn id(self) -> HostId {
        HostId {
            edge: HostEdge::Executor,
            index: self as usize,
        }
    }
}

impl From<ExecutorHost> for HostId {
    fn from(entry: ExecutorHost) -> Self {
        entry.id()
    }
}

/// One third-party entry of `sqlbuild._native_host`, indexing its `ENTRIES`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThirdPartyHost {
    AssetSelectionCommands,
    CheckMetadataExcludedKeys,
    CloneCommand,
    CompletedExecutionStatuses,
    DagsterAssetEnabledField,
    DagsterAssetNodeKinds,
    DagsterCheckEnabledField,
    DagsterCheckNameField,
    DagsterDirectKindNodeKinds,
    DefaultSelectableNodeKinds,
    ExplicitSelectionFlags,
    InsufficientCheckStatus,
    LiveEventCommands,
    LoaderNodeKind,
    LoadSelectableNodeKinds,
    MaterializableNodeKinds,
    ModelNodeKind,
    ScenarioCheckKind,
    SourceNodeKind,
    StandaloneCheckCommands,
    SuccessExecutionStatus,
    ViewMaterializationType,
    WarningCheckSeverity,
    DagsterDagInputError,
    LoadDbtYaml,
    RenderSelectedDbtProfileOutput,
    DbtProfileError,
    DltIntegrationKind,
    DltIntegrationError,
    DltLoaderFunction,
    DltResourceConfig,
    DltSourceConfig,
    IngestrIntegrationKind,
    IngestrIntegrationError,
    IngestrLoaderFunction,
    IngestrSourceConfig,
    RiversAssetNodeKinds,
}

impl ThirdPartyHost {
    /// The entry's position in `HOST_ENTRIES`, usable in constant tables.
    pub const fn id(self) -> HostId {
        HostId {
            edge: HostEdge::ThirdParty,
            index: self as usize,
        }
    }
}

impl From<ThirdPartyHost> for HostId {
    fn from(entry: ThirdPartyHost) -> Self {
        entry.id()
    }
}

/// One facade entry of `sqlbuild._native_host`, indexing its `ENTRIES`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FacadeHost {
    PackageFile,
    DispatchWithComputeLogs,
    ProviderSettingsInputs,
    StartupInputs,
    CompileReuseRequest,
    AuditEvaluationMode,
    AuditOutcome,
    AuditSeverity,
    ThresholdOperator,
    AttachedAuditTargetKind,
    FunctionLanguage,
    SqlTestMode,
    CursorAlgebraError,
    SharedInputError,
}

impl FacadeHost {
    /// The entry's position in `HOST_ENTRIES`, usable in constant tables.
    pub const fn id(self) -> HostId {
        HostId {
            edge: HostEdge::Facade,
            index: self as usize,
        }
    }
}

impl From<FacadeHost> for HostId {
    fn from(entry: FacadeHost) -> Self {
        entry.id()
    }
}
