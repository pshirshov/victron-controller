// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'
import {PinnedRegister, PinnedRegister_UEBACodec} from './PinnedRegister'
import {Sensors, Sensors_UEBACodec} from './Sensors'
import {CoresState, CoresState_UEBACodec} from './CoresState'
import {Diagnostics, Diagnostics_UEBACodec} from './Diagnostics'
import {Knobs, Knobs_UEBACodec} from './Knobs'
import {Bookkeeping, Bookkeeping_UEBACodec} from './Bookkeeping'
import {Timers, Timers_UEBACodec} from './Timers'
import {SocChart, SocChart_UEBACodec} from './SocChart'
import {WeatherSocActive, WeatherSocActive_UEBACodec} from './WeatherSocActive'
import {SensorMeta, SensorMeta_UEBACodec} from './SensorMeta'
import {Actuated, Actuated_UEBACodec} from './Actuated'
import {ZappiDrainState, ZappiDrainState_UEBACodec} from './ZappiDrainState'
import {TypedSensors, TypedSensors_UEBACodec} from './TypedSensors'
import {Forecasts, Forecasts_UEBACodec} from './Forecasts'
import {ScheduledActions, ScheduledActions_UEBACodec} from './ScheduledActions'
import {Decisions, Decisions_UEBACodec} from './Decisions'

export class WorldSnapshot implements BaboonGeneratedLatest {
    private readonly _captured_at_epoch_ms: bigint;
    private readonly _captured_at_naive_iso: string;
    private readonly _sensors: Sensors;
    private readonly _sensors_meta: Record<string, SensorMeta>;
    private readonly _actuated: Actuated;
    private readonly _knobs: Knobs;
    private readonly _bookkeeping: Bookkeeping;
    private readonly _forecasts: Forecasts;
    private readonly _decisions: Decisions;
    private readonly _cores_state: CoresState;
    private readonly _timers: Timers;
    private readonly _soc_chart: SocChart;
    private readonly _scheduled_actions: ScheduledActions;
    private readonly _pinned_registers: Array<PinnedRegister>;
    private readonly _zappi_drain_state: ZappiDrainState;
    private readonly _typed_sensors: TypedSensors;
    private readonly _weather_soc_active: WeatherSocActive | undefined;
    private readonly _diagnostics: Diagnostics;

    constructor(captured_at_epoch_ms: bigint, captured_at_naive_iso: string, sensors: Sensors, sensors_meta: Record<string, SensorMeta>, actuated: Actuated, knobs: Knobs, bookkeeping: Bookkeeping, forecasts: Forecasts, decisions: Decisions, cores_state: CoresState, timers: Timers, soc_chart: SocChart, scheduled_actions: ScheduledActions, pinned_registers: Array<PinnedRegister>, zappi_drain_state: ZappiDrainState, typed_sensors: TypedSensors, weather_soc_active: WeatherSocActive | undefined, diagnostics: Diagnostics) {
        this._captured_at_epoch_ms = captured_at_epoch_ms
        this._captured_at_naive_iso = captured_at_naive_iso
        this._sensors = sensors
        this._sensors_meta = sensors_meta
        this._actuated = actuated
        this._knobs = knobs
        this._bookkeeping = bookkeeping
        this._forecasts = forecasts
        this._decisions = decisions
        this._cores_state = cores_state
        this._timers = timers
        this._soc_chart = soc_chart
        this._scheduled_actions = scheduled_actions
        this._pinned_registers = pinned_registers
        this._zappi_drain_state = zappi_drain_state
        this._typed_sensors = typed_sensors
        this._weather_soc_active = weather_soc_active
        this._diagnostics = diagnostics
    }

    public get captured_at_epoch_ms(): bigint {
        return this._captured_at_epoch_ms;
    }
    public get captured_at_naive_iso(): string {
        return this._captured_at_naive_iso;
    }
    public get sensors(): Sensors {
        return this._sensors;
    }
    public get sensors_meta(): Record<string, SensorMeta> {
        return this._sensors_meta;
    }
    public get actuated(): Actuated {
        return this._actuated;
    }
    public get knobs(): Knobs {
        return this._knobs;
    }
    public get bookkeeping(): Bookkeeping {
        return this._bookkeeping;
    }
    public get forecasts(): Forecasts {
        return this._forecasts;
    }
    public get decisions(): Decisions {
        return this._decisions;
    }
    public get cores_state(): CoresState {
        return this._cores_state;
    }
    public get timers(): Timers {
        return this._timers;
    }
    public get soc_chart(): SocChart {
        return this._soc_chart;
    }
    public get scheduled_actions(): ScheduledActions {
        return this._scheduled_actions;
    }
    public get pinned_registers(): Array<PinnedRegister> {
        return this._pinned_registers;
    }
    public get zappi_drain_state(): ZappiDrainState {
        return this._zappi_drain_state;
    }
    public get typed_sensors(): TypedSensors {
        return this._typed_sensors;
    }
    public get weather_soc_active(): WeatherSocActive | undefined {
        return this._weather_soc_active;
    }
    public get diagnostics(): Diagnostics {
        return this._diagnostics;
    }

    public toJSON(): Record<string, unknown> {
        return {
            captured_at_epoch_ms: this._captured_at_epoch_ms,
            captured_at_naive_iso: this._captured_at_naive_iso,
            sensors: this._sensors,
            sensors_meta: this._sensors_meta,
            actuated: this._actuated,
            knobs: this._knobs,
            bookkeeping: this._bookkeeping,
            forecasts: this._forecasts,
            decisions: this._decisions,
            cores_state: this._cores_state,
            timers: this._timers,
            soc_chart: this._soc_chart,
            scheduled_actions: this._scheduled_actions,
            pinned_registers: this._pinned_registers,
            zappi_drain_state: this._zappi_drain_state,
            typed_sensors: this._typed_sensors,
            weather_soc_active: this._weather_soc_active !== undefined ? this._weather_soc_active : undefined,
            diagnostics: this._diagnostics
        };
    }

    public with(overrides: {captured_at_epoch_ms?: bigint; captured_at_naive_iso?: string; sensors?: Sensors; sensors_meta?: Record<string, SensorMeta>; actuated?: Actuated; knobs?: Knobs; bookkeeping?: Bookkeeping; forecasts?: Forecasts; decisions?: Decisions; cores_state?: CoresState; timers?: Timers; soc_chart?: SocChart; scheduled_actions?: ScheduledActions; pinned_registers?: Array<PinnedRegister>; zappi_drain_state?: ZappiDrainState; typed_sensors?: TypedSensors; weather_soc_active?: WeatherSocActive | undefined; diagnostics?: Diagnostics}): WorldSnapshot {
        return new WorldSnapshot(
            'captured_at_epoch_ms' in overrides ? overrides.captured_at_epoch_ms! : this._captured_at_epoch_ms,
            'captured_at_naive_iso' in overrides ? overrides.captured_at_naive_iso! : this._captured_at_naive_iso,
            'sensors' in overrides ? overrides.sensors! : this._sensors,
            'sensors_meta' in overrides ? overrides.sensors_meta! : this._sensors_meta,
            'actuated' in overrides ? overrides.actuated! : this._actuated,
            'knobs' in overrides ? overrides.knobs! : this._knobs,
            'bookkeeping' in overrides ? overrides.bookkeeping! : this._bookkeeping,
            'forecasts' in overrides ? overrides.forecasts! : this._forecasts,
            'decisions' in overrides ? overrides.decisions! : this._decisions,
            'cores_state' in overrides ? overrides.cores_state! : this._cores_state,
            'timers' in overrides ? overrides.timers! : this._timers,
            'soc_chart' in overrides ? overrides.soc_chart! : this._soc_chart,
            'scheduled_actions' in overrides ? overrides.scheduled_actions! : this._scheduled_actions,
            'pinned_registers' in overrides ? overrides.pinned_registers! : this._pinned_registers,
            'zappi_drain_state' in overrides ? overrides.zappi_drain_state! : this._zappi_drain_state,
            'typed_sensors' in overrides ? overrides.typed_sensors! : this._typed_sensors,
            'weather_soc_active' in overrides ? overrides.weather_soc_active! : this._weather_soc_active,
            'diagnostics' in overrides ? overrides.diagnostics! : this._diagnostics
        );
    }

    public static fromPlain(obj: {captured_at_epoch_ms: bigint; captured_at_naive_iso: string; sensors: Sensors; sensors_meta: Record<string, SensorMeta>; actuated: Actuated; knobs: Knobs; bookkeeping: Bookkeeping; forecasts: Forecasts; decisions: Decisions; cores_state: CoresState; timers: Timers; soc_chart: SocChart; scheduled_actions: ScheduledActions; pinned_registers: Array<PinnedRegister>; zappi_drain_state: ZappiDrainState; typed_sensors: TypedSensors; weather_soc_active: WeatherSocActive | undefined; diagnostics: Diagnostics}): WorldSnapshot {
        return new WorldSnapshot(
            obj.captured_at_epoch_ms,
            obj.captured_at_naive_iso,
            obj.sensors,
            obj.sensors_meta,
            obj.actuated,
            obj.knobs,
            obj.bookkeeping,
            obj.forecasts,
            obj.decisions,
            obj.cores_state,
            obj.timers,
            obj.soc_chart,
            obj.scheduled_actions,
            obj.pinned_registers,
            obj.zappi_drain_state,
            obj.typed_sensors,
            obj.weather_soc_active,
            obj.diagnostics
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return WorldSnapshot.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return WorldSnapshot.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#WorldSnapshot'
    public baboonTypeIdentifier() {
        return WorldSnapshot.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.3.0"]
    public baboonSameInVersions() {
        return WorldSnapshot.BaboonSameInVersions
    }
    public static binCodec(): WorldSnapshot_UEBACodec {
        return WorldSnapshot_UEBACodec.instance
    }
}

export class WorldSnapshot_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: WorldSnapshot, writer: BaboonBinWriter): unknown {
        if (this !== WorldSnapshot_UEBACodec.lazyInstance.value) {
          return WorldSnapshot_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            BinTools.writeI64(buffer, value.captured_at_epoch_ms);
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeString(buffer, value.captured_at_naive_iso);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                Sensors_UEBACodec.instance.encode(ctx, value.sensors, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                {
                const entries = Object.entries(value.sensors_meta);
                BinTools.writeI32(buffer, entries.length);
                for (const [k, v] of entries) {
                    BinTools.writeString(buffer, k);
                    SensorMeta_UEBACodec.instance.encode(ctx, v, buffer);
                }
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                Actuated_UEBACodec.instance.encode(ctx, value.actuated, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            Knobs_UEBACodec.instance.encode(ctx, value.knobs, buffer);
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                Bookkeeping_UEBACodec.instance.encode(ctx, value.bookkeeping, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                Forecasts_UEBACodec.instance.encode(ctx, value.forecasts, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                Decisions_UEBACodec.instance.encode(ctx, value.decisions, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                CoresState_UEBACodec.instance.encode(ctx, value.cores_state, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                Timers_UEBACodec.instance.encode(ctx, value.timers, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                SocChart_UEBACodec.instance.encode(ctx, value.soc_chart, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ScheduledActions_UEBACodec.instance.encode(ctx, value.scheduled_actions, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeI32(buffer, Array.from(value.pinned_registers).length);
            for (const item of value.pinned_registers) {
                PinnedRegister_UEBACodec.instance.encode(ctx, item, buffer);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ZappiDrainState_UEBACodec.instance.encode(ctx, value.zappi_drain_state, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                TypedSensors_UEBACodec.instance.encode(ctx, value.typed_sensors, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.weather_soc_active === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                WeatherSocActive_UEBACodec.instance.encode(ctx, value.weather_soc_active, buffer);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            Diagnostics_UEBACodec.instance.encode(ctx, value.diagnostics, buffer);
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeI64(writer, value.captured_at_epoch_ms);
            BinTools.writeString(writer, value.captured_at_naive_iso);
            Sensors_UEBACodec.instance.encode(ctx, value.sensors, writer);
            {
                const entries = Object.entries(value.sensors_meta);
                BinTools.writeI32(writer, entries.length);
                for (const [k, v] of entries) {
                    BinTools.writeString(writer, k);
                    SensorMeta_UEBACodec.instance.encode(ctx, v, writer);
                }
            }
            Actuated_UEBACodec.instance.encode(ctx, value.actuated, writer);
            Knobs_UEBACodec.instance.encode(ctx, value.knobs, writer);
            Bookkeeping_UEBACodec.instance.encode(ctx, value.bookkeeping, writer);
            Forecasts_UEBACodec.instance.encode(ctx, value.forecasts, writer);
            Decisions_UEBACodec.instance.encode(ctx, value.decisions, writer);
            CoresState_UEBACodec.instance.encode(ctx, value.cores_state, writer);
            Timers_UEBACodec.instance.encode(ctx, value.timers, writer);
            SocChart_UEBACodec.instance.encode(ctx, value.soc_chart, writer);
            ScheduledActions_UEBACodec.instance.encode(ctx, value.scheduled_actions, writer);
            BinTools.writeI32(writer, Array.from(value.pinned_registers).length);
            for (const item of value.pinned_registers) {
                PinnedRegister_UEBACodec.instance.encode(ctx, item, writer);
            }
            ZappiDrainState_UEBACodec.instance.encode(ctx, value.zappi_drain_state, writer);
            TypedSensors_UEBACodec.instance.encode(ctx, value.typed_sensors, writer);
            if (value.weather_soc_active === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                WeatherSocActive_UEBACodec.instance.encode(ctx, value.weather_soc_active, writer);
            }
            Diagnostics_UEBACodec.instance.encode(ctx, value.diagnostics, writer);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): WorldSnapshot {
        if (this !== WorldSnapshot_UEBACodec .lazyInstance.value) {
            return WorldSnapshot_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 15; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const captured_at_epoch_ms = BinTools.readI64(reader);
        const captured_at_naive_iso = BinTools.readString(reader);
        const sensors = Sensors_UEBACodec.instance.decode(ctx, reader);
        const sensors_meta = Object.fromEntries(Array.from({ length: BinTools.readI32(reader) }, () => [BinTools.readString(reader), SensorMeta_UEBACodec.instance.decode(ctx, reader)] as const));
        const actuated = Actuated_UEBACodec.instance.decode(ctx, reader);
        const knobs = Knobs_UEBACodec.instance.decode(ctx, reader);
        const bookkeeping = Bookkeeping_UEBACodec.instance.decode(ctx, reader);
        const forecasts = Forecasts_UEBACodec.instance.decode(ctx, reader);
        const decisions = Decisions_UEBACodec.instance.decode(ctx, reader);
        const cores_state = CoresState_UEBACodec.instance.decode(ctx, reader);
        const timers = Timers_UEBACodec.instance.decode(ctx, reader);
        const soc_chart = SocChart_UEBACodec.instance.decode(ctx, reader);
        const scheduled_actions = ScheduledActions_UEBACodec.instance.decode(ctx, reader);
        const pinned_registers = Array.from({ length: BinTools.readI32(reader) }, () => PinnedRegister_UEBACodec.instance.decode(ctx, reader));
        const zappi_drain_state = ZappiDrainState_UEBACodec.instance.decode(ctx, reader);
        const typed_sensors = TypedSensors_UEBACodec.instance.decode(ctx, reader);
        const weather_soc_active = (BinTools.readByte(reader) === 0 ? undefined : WeatherSocActive_UEBACodec.instance.decode(ctx, reader));
        const diagnostics = Diagnostics_UEBACodec.instance.decode(ctx, reader);
        return new WorldSnapshot(
            captured_at_epoch_ms,
            captured_at_naive_iso,
            sensors,
            sensors_meta,
            actuated,
            knobs,
            bookkeeping,
            forecasts,
            decisions,
            cores_state,
            timers,
            soc_chart,
            scheduled_actions,
            pinned_registers,
            zappi_drain_state,
            typed_sensors,
            weather_soc_active,
            diagnostics,
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return WorldSnapshot_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return WorldSnapshot_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#WorldSnapshot'
    public baboonTypeIdentifier() {
        return WorldSnapshot_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new WorldSnapshot_UEBACodec())
    public static get instance(): WorldSnapshot_UEBACodec {
        return WorldSnapshot_UEBACodec.lazyInstance.value
    }
}