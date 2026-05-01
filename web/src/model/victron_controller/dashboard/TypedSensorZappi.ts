// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'
import {Freshness, Freshness_UEBACodec} from './Freshness'

export class TypedSensorZappi implements BaboonGeneratedLatest {
    private readonly _mode: string | undefined;
    private readonly _status: string | undefined;
    private readonly _plug_state: string | undefined;
    private readonly _freshness: Freshness;
    private readonly _since_epoch_ms: bigint;
    private readonly _raw_json: string | undefined;
    private readonly _cadence_ms: bigint;
    private readonly _staleness_ms: bigint;
    private readonly _origin: string;
    private readonly _identifier: string;

    constructor(mode: string | undefined, status: string | undefined, plug_state: string | undefined, freshness: Freshness, since_epoch_ms: bigint, raw_json: string | undefined, cadence_ms: bigint, staleness_ms: bigint, origin: string, identifier: string) {
        this._mode = mode
        this._status = status
        this._plug_state = plug_state
        this._freshness = freshness
        this._since_epoch_ms = since_epoch_ms
        this._raw_json = raw_json
        this._cadence_ms = cadence_ms
        this._staleness_ms = staleness_ms
        this._origin = origin
        this._identifier = identifier
    }

    public get mode(): string | undefined {
        return this._mode;
    }
    public get status(): string | undefined {
        return this._status;
    }
    public get plug_state(): string | undefined {
        return this._plug_state;
    }
    public get freshness(): Freshness {
        return this._freshness;
    }
    public get since_epoch_ms(): bigint {
        return this._since_epoch_ms;
    }
    public get raw_json(): string | undefined {
        return this._raw_json;
    }
    public get cadence_ms(): bigint {
        return this._cadence_ms;
    }
    public get staleness_ms(): bigint {
        return this._staleness_ms;
    }
    public get origin(): string {
        return this._origin;
    }
    public get identifier(): string {
        return this._identifier;
    }

    public toJSON(): Record<string, unknown> {
        return {
            mode: this._mode !== undefined ? this._mode : undefined,
            status: this._status !== undefined ? this._status : undefined,
            plug_state: this._plug_state !== undefined ? this._plug_state : undefined,
            freshness: this._freshness,
            since_epoch_ms: this._since_epoch_ms,
            raw_json: this._raw_json !== undefined ? this._raw_json : undefined,
            cadence_ms: this._cadence_ms,
            staleness_ms: this._staleness_ms,
            origin: this._origin,
            identifier: this._identifier
        };
    }

    public with(overrides: {mode?: string | undefined; status?: string | undefined; plug_state?: string | undefined; freshness?: Freshness; since_epoch_ms?: bigint; raw_json?: string | undefined; cadence_ms?: bigint; staleness_ms?: bigint; origin?: string; identifier?: string}): TypedSensorZappi {
        return new TypedSensorZappi(
            'mode' in overrides ? overrides.mode! : this._mode,
            'status' in overrides ? overrides.status! : this._status,
            'plug_state' in overrides ? overrides.plug_state! : this._plug_state,
            'freshness' in overrides ? overrides.freshness! : this._freshness,
            'since_epoch_ms' in overrides ? overrides.since_epoch_ms! : this._since_epoch_ms,
            'raw_json' in overrides ? overrides.raw_json! : this._raw_json,
            'cadence_ms' in overrides ? overrides.cadence_ms! : this._cadence_ms,
            'staleness_ms' in overrides ? overrides.staleness_ms! : this._staleness_ms,
            'origin' in overrides ? overrides.origin! : this._origin,
            'identifier' in overrides ? overrides.identifier! : this._identifier
        );
    }

    public static fromPlain(obj: {mode: string | undefined; status: string | undefined; plug_state: string | undefined; freshness: Freshness; since_epoch_ms: bigint; raw_json: string | undefined; cadence_ms: bigint; staleness_ms: bigint; origin: string; identifier: string}): TypedSensorZappi {
        return new TypedSensorZappi(
            obj.mode,
            obj.status,
            obj.plug_state,
            obj.freshness,
            obj.since_epoch_ms,
            obj.raw_json,
            obj.cadence_ms,
            obj.staleness_ms,
            obj.origin,
            obj.identifier
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return TypedSensorZappi.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return TypedSensorZappi.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#TypedSensorZappi'
    public baboonTypeIdentifier() {
        return TypedSensorZappi.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.3.0"]
    public baboonSameInVersions() {
        return TypedSensorZappi.BaboonSameInVersions
    }
    public static binCodec(): TypedSensorZappi_UEBACodec {
        return TypedSensorZappi_UEBACodec.instance
    }
}

export class TypedSensorZappi_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: TypedSensorZappi, writer: BaboonBinWriter): unknown {
        if (this !== TypedSensorZappi_UEBACodec.lazyInstance.value) {
          return TypedSensorZappi_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.mode === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeString(buffer, value.mode);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.status === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeString(buffer, value.status);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.plug_state === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeString(buffer, value.plug_state);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            Freshness_UEBACodec.instance.encode(ctx, value.freshness, buffer);
            BinTools.writeI64(buffer, value.since_epoch_ms);
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.raw_json === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeString(buffer, value.raw_json);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            BinTools.writeI64(buffer, value.cadence_ms);
            BinTools.writeI64(buffer, value.staleness_ms);
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeString(buffer, value.origin);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeString(buffer, value.identifier);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            if (value.mode === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeString(writer, value.mode);
            }
            if (value.status === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeString(writer, value.status);
            }
            if (value.plug_state === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeString(writer, value.plug_state);
            }
            Freshness_UEBACodec.instance.encode(ctx, value.freshness, writer);
            BinTools.writeI64(writer, value.since_epoch_ms);
            if (value.raw_json === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeString(writer, value.raw_json);
            }
            BinTools.writeI64(writer, value.cadence_ms);
            BinTools.writeI64(writer, value.staleness_ms);
            BinTools.writeString(writer, value.origin);
            BinTools.writeString(writer, value.identifier);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): TypedSensorZappi {
        if (this !== TypedSensorZappi_UEBACodec .lazyInstance.value) {
            return TypedSensorZappi_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 6; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const mode = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readString(reader));
        const status = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readString(reader));
        const plug_state = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readString(reader));
        const freshness = Freshness_UEBACodec.instance.decode(ctx, reader);
        const since_epoch_ms = BinTools.readI64(reader);
        const raw_json = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readString(reader));
        const cadence_ms = BinTools.readI64(reader);
        const staleness_ms = BinTools.readI64(reader);
        const origin = BinTools.readString(reader);
        const identifier = BinTools.readString(reader);
        return new TypedSensorZappi(
            mode,
            status,
            plug_state,
            freshness,
            since_epoch_ms,
            raw_json,
            cadence_ms,
            staleness_ms,
            origin,
            identifier,
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return TypedSensorZappi_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return TypedSensorZappi_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#TypedSensorZappi'
    public baboonTypeIdentifier() {
        return TypedSensorZappi_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new TypedSensorZappi_UEBACodec())
    public static get instance(): TypedSensorZappi_UEBACodec {
        return TypedSensorZappi_UEBACodec.lazyInstance.value
    }
}