// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'
import {Freshness, Freshness_UEBACodec} from './Freshness'

export class TypedSensorEnum implements BaboonGeneratedLatest {
    private readonly _value: string | undefined;
    private readonly _freshness: Freshness;
    private readonly _since_epoch_ms: bigint;
    private readonly _raw_json: string | undefined;
    private readonly _cadence_ms: bigint;
    private readonly _staleness_ms: bigint;
    private readonly _origin: string;
    private readonly _identifier: string;

    constructor(value: string | undefined, freshness: Freshness, since_epoch_ms: bigint, raw_json: string | undefined, cadence_ms: bigint, staleness_ms: bigint, origin: string, identifier: string) {
        this._value = value
        this._freshness = freshness
        this._since_epoch_ms = since_epoch_ms
        this._raw_json = raw_json
        this._cadence_ms = cadence_ms
        this._staleness_ms = staleness_ms
        this._origin = origin
        this._identifier = identifier
    }

    public get value(): string | undefined {
        return this._value;
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
            value: this._value !== undefined ? this._value : undefined,
            freshness: this._freshness,
            since_epoch_ms: this._since_epoch_ms,
            raw_json: this._raw_json !== undefined ? this._raw_json : undefined,
            cadence_ms: this._cadence_ms,
            staleness_ms: this._staleness_ms,
            origin: this._origin,
            identifier: this._identifier
        };
    }

    public with(overrides: {value?: string | undefined; freshness?: Freshness; since_epoch_ms?: bigint; raw_json?: string | undefined; cadence_ms?: bigint; staleness_ms?: bigint; origin?: string; identifier?: string}): TypedSensorEnum {
        return new TypedSensorEnum(
            'value' in overrides ? overrides.value! : this._value,
            'freshness' in overrides ? overrides.freshness! : this._freshness,
            'since_epoch_ms' in overrides ? overrides.since_epoch_ms! : this._since_epoch_ms,
            'raw_json' in overrides ? overrides.raw_json! : this._raw_json,
            'cadence_ms' in overrides ? overrides.cadence_ms! : this._cadence_ms,
            'staleness_ms' in overrides ? overrides.staleness_ms! : this._staleness_ms,
            'origin' in overrides ? overrides.origin! : this._origin,
            'identifier' in overrides ? overrides.identifier! : this._identifier
        );
    }

    public static fromPlain(obj: {value: string | undefined; freshness: Freshness; since_epoch_ms: bigint; raw_json: string | undefined; cadence_ms: bigint; staleness_ms: bigint; origin: string; identifier: string}): TypedSensorEnum {
        return new TypedSensorEnum(
            obj.value,
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
        return TypedSensorEnum.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return TypedSensorEnum.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#TypedSensorEnum'
    public baboonTypeIdentifier() {
        return TypedSensorEnum.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.3.0"]
    public baboonSameInVersions() {
        return TypedSensorEnum.BaboonSameInVersions
    }
    public static binCodec(): TypedSensorEnum_UEBACodec {
        return TypedSensorEnum_UEBACodec.instance
    }
}

export class TypedSensorEnum_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: TypedSensorEnum, writer: BaboonBinWriter): unknown {
        if (this !== TypedSensorEnum_UEBACodec.lazyInstance.value) {
          return TypedSensorEnum_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.value === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeString(buffer, value.value);
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
            if (value.value === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeString(writer, value.value);
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
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): TypedSensorEnum {
        if (this !== TypedSensorEnum_UEBACodec .lazyInstance.value) {
            return TypedSensorEnum_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 4; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const value = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readString(reader));
        const freshness = Freshness_UEBACodec.instance.decode(ctx, reader);
        const since_epoch_ms = BinTools.readI64(reader);
        const raw_json = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readString(reader));
        const cadence_ms = BinTools.readI64(reader);
        const staleness_ms = BinTools.readI64(reader);
        const origin = BinTools.readString(reader);
        const identifier = BinTools.readString(reader);
        return new TypedSensorEnum(
            value,
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
        return TypedSensorEnum_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return TypedSensorEnum_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#TypedSensorEnum'
    public baboonTypeIdentifier() {
        return TypedSensorEnum_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new TypedSensorEnum_UEBACodec())
    public static get instance(): TypedSensorEnum_UEBACodec {
        return TypedSensorEnum_UEBACodec.lazyInstance.value
    }
}