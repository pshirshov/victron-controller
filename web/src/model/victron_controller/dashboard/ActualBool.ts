// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'
import {Freshness, Freshness_UEBACodec} from './Freshness'

export class ActualBool implements BaboonGeneratedLatest {
    private readonly _value: boolean | undefined;
    private readonly _freshness: Freshness;
    private readonly _since_epoch_ms: bigint;

    constructor(value: boolean | undefined, freshness: Freshness, since_epoch_ms: bigint) {
        this._value = value
        this._freshness = freshness
        this._since_epoch_ms = since_epoch_ms
    }

    public get value(): boolean | undefined {
        return this._value;
    }
    public get freshness(): Freshness {
        return this._freshness;
    }
    public get since_epoch_ms(): bigint {
        return this._since_epoch_ms;
    }

    public toJSON(): Record<string, unknown> {
        return {
            value: this._value !== undefined ? this._value : undefined,
            freshness: this._freshness,
            since_epoch_ms: this._since_epoch_ms
        };
    }

    public with(overrides: {value?: boolean | undefined; freshness?: Freshness; since_epoch_ms?: bigint}): ActualBool {
        return new ActualBool(
            'value' in overrides ? overrides.value! : this._value,
            'freshness' in overrides ? overrides.freshness! : this._freshness,
            'since_epoch_ms' in overrides ? overrides.since_epoch_ms! : this._since_epoch_ms
        );
    }

    public static fromPlain(obj: {value: boolean | undefined; freshness: Freshness; since_epoch_ms: bigint}): ActualBool {
        return new ActualBool(
            obj.value,
            obj.freshness,
            obj.since_epoch_ms
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return ActualBool.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return ActualBool.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#ActualBool'
    public baboonTypeIdentifier() {
        return ActualBool.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.3.0"]
    public baboonSameInVersions() {
        return ActualBool.BaboonSameInVersions
    }
    public static binCodec(): ActualBool_UEBACodec {
        return ActualBool_UEBACodec.instance
    }
}

export class ActualBool_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: ActualBool, writer: BaboonBinWriter): unknown {
        if (this !== ActualBool_UEBACodec.lazyInstance.value) {
          return ActualBool_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
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
                BinTools.writeBool(buffer, value.value);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            Freshness_UEBACodec.instance.encode(ctx, value.freshness, buffer);
            BinTools.writeI64(buffer, value.since_epoch_ms);
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            if (value.value === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeBool(writer, value.value);
            }
            Freshness_UEBACodec.instance.encode(ctx, value.freshness, writer);
            BinTools.writeI64(writer, value.since_epoch_ms);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): ActualBool {
        if (this !== ActualBool_UEBACodec .lazyInstance.value) {
            return ActualBool_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 1; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const value = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readBool(reader));
        const freshness = Freshness_UEBACodec.instance.decode(ctx, reader);
        const since_epoch_ms = BinTools.readI64(reader);
        return new ActualBool(
            value,
            freshness,
            since_epoch_ms,
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return ActualBool_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return ActualBool_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#ActualBool'
    public baboonTypeIdentifier() {
        return ActualBool_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new ActualBool_UEBACodec())
    public static get instance(): ActualBool_UEBACodec {
        return ActualBool_UEBACodec.lazyInstance.value
    }
}