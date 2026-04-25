// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'
import {Knobs, Knobs_UEBACodec} from './Knobs'
import {Bookkeeping, Bookkeeping_UEBACodec} from './Bookkeeping'
import {SensorMeta, SensorMeta_UEBACodec} from './SensorMeta'
import {Actuated, Actuated_UEBACodec} from './Actuated'
import {Sensors, Sensors_UEBACodec} from './Sensors'
import {Forecasts, Forecasts_UEBACodec} from './Forecasts'
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

    constructor(captured_at_epoch_ms: bigint, captured_at_naive_iso: string, sensors: Sensors, sensors_meta: Record<string, SensorMeta>, actuated: Actuated, knobs: Knobs, bookkeeping: Bookkeeping, forecasts: Forecasts, decisions: Decisions) {
        this._captured_at_epoch_ms = captured_at_epoch_ms
        this._captured_at_naive_iso = captured_at_naive_iso
        this._sensors = sensors
        this._sensors_meta = sensors_meta
        this._actuated = actuated
        this._knobs = knobs
        this._bookkeeping = bookkeeping
        this._forecasts = forecasts
        this._decisions = decisions
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
            decisions: this._decisions
        };
    }

    public with(overrides: {captured_at_epoch_ms?: bigint; captured_at_naive_iso?: string; sensors?: Sensors; sensors_meta?: Record<string, SensorMeta>; actuated?: Actuated; knobs?: Knobs; bookkeeping?: Bookkeeping; forecasts?: Forecasts; decisions?: Decisions}): WorldSnapshot {
        return new WorldSnapshot(
            'captured_at_epoch_ms' in overrides ? overrides.captured_at_epoch_ms! : this._captured_at_epoch_ms,
            'captured_at_naive_iso' in overrides ? overrides.captured_at_naive_iso! : this._captured_at_naive_iso,
            'sensors' in overrides ? overrides.sensors! : this._sensors,
            'sensors_meta' in overrides ? overrides.sensors_meta! : this._sensors_meta,
            'actuated' in overrides ? overrides.actuated! : this._actuated,
            'knobs' in overrides ? overrides.knobs! : this._knobs,
            'bookkeeping' in overrides ? overrides.bookkeeping! : this._bookkeeping,
            'forecasts' in overrides ? overrides.forecasts! : this._forecasts,
            'decisions' in overrides ? overrides.decisions! : this._decisions
        );
    }

    public static fromPlain(obj: {captured_at_epoch_ms: bigint; captured_at_naive_iso: string; sensors: Sensors; sensors_meta: Record<string, SensorMeta>; actuated: Actuated; knobs: Knobs; bookkeeping: Bookkeeping; forecasts: Forecasts; decisions: Decisions}): WorldSnapshot {
        return new WorldSnapshot(
            obj.captured_at_epoch_ms,
            obj.captured_at_naive_iso,
            obj.sensors,
            obj.sensors_meta,
            obj.actuated,
            obj.knobs,
            obj.bookkeeping,
            obj.forecasts,
            obj.decisions
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
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
    public static readonly BaboonSameInVersions = ["0.2.0"]
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
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): WorldSnapshot {
        if (this !== WorldSnapshot_UEBACodec .lazyInstance.value) {
            return WorldSnapshot_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 7; i++) {
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
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
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