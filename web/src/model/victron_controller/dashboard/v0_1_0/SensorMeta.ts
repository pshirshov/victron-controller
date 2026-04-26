// @ts-nocheck
import {BaboonGenerated, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../../BaboonSharedRuntime'

export class SensorMeta implements BaboonGenerated {
    private readonly _origin: string;
    private readonly _identifier: string;
    private readonly _cadence_ms: bigint;
    private readonly _staleness_ms: bigint;

    constructor(origin: string, identifier: string, cadence_ms: bigint, staleness_ms: bigint) {
        this._origin = origin
        this._identifier = identifier
        this._cadence_ms = cadence_ms
        this._staleness_ms = staleness_ms
    }

    public get origin(): string {
        return this._origin;
    }
    public get identifier(): string {
        return this._identifier;
    }
    public get cadence_ms(): bigint {
        return this._cadence_ms;
    }
    public get staleness_ms(): bigint {
        return this._staleness_ms;
    }

    public toJSON(): Record<string, unknown> {
        return {
            origin: this._origin,
            identifier: this._identifier,
            cadence_ms: this._cadence_ms,
            staleness_ms: this._staleness_ms
        };
    }

    public with(overrides: {origin?: string; identifier?: string; cadence_ms?: bigint; staleness_ms?: bigint}): SensorMeta {
        return new SensorMeta(
            'origin' in overrides ? overrides.origin! : this._origin,
            'identifier' in overrides ? overrides.identifier! : this._identifier,
            'cadence_ms' in overrides ? overrides.cadence_ms! : this._cadence_ms,
            'staleness_ms' in overrides ? overrides.staleness_ms! : this._staleness_ms
        );
    }

    public static fromPlain(obj: {origin: string; identifier: string; cadence_ms: bigint; staleness_ms: bigint}): SensorMeta {
        return new SensorMeta(
            obj.origin,
            obj.identifier,
            obj.cadence_ms,
            obj.staleness_ms
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return SensorMeta.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return SensorMeta.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#SensorMeta'
    public baboonTypeIdentifier() {
        return SensorMeta.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0", "0.2.0", "0.3.0"]
    public baboonSameInVersions() {
        return SensorMeta.BaboonSameInVersions
    }
    public static binCodec(): SensorMeta_UEBACodec {
        return SensorMeta_UEBACodec.instance
    }
}

/** @deprecated Version 0.1.0 is deprecated, you should migrate to 0.3.0 */
export class SensorMeta_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: SensorMeta, writer: BaboonBinWriter): unknown {
        if (this !== SensorMeta_UEBACodec.lazyInstance.value) {
          return SensorMeta_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
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
            BinTools.writeI64(buffer, value.cadence_ms);
            BinTools.writeI64(buffer, value.staleness_ms);
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeString(writer, value.origin);
            BinTools.writeString(writer, value.identifier);
            BinTools.writeI64(writer, value.cadence_ms);
            BinTools.writeI64(writer, value.staleness_ms);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): SensorMeta {
        if (this !== SensorMeta_UEBACodec .lazyInstance.value) {
            return SensorMeta_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 2; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const origin = BinTools.readString(reader);
        const identifier = BinTools.readString(reader);
        const cadence_ms = BinTools.readI64(reader);
        const staleness_ms = BinTools.readI64(reader);
        return new SensorMeta(
            origin,
            identifier,
            cadence_ms,
            staleness_ms,
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return SensorMeta_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return SensorMeta_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#SensorMeta'
    public baboonTypeIdentifier() {
        return SensorMeta_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new SensorMeta_UEBACodec())
    public static get instance(): SensorMeta_UEBACodec {
        return SensorMeta_UEBACodec.lazyInstance.value
    }
}