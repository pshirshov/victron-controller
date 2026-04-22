// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'
import {Freshness, Freshness_UEBACodec} from './Freshness'

export class ActualF64 implements BaboonGeneratedLatest {
    private readonly _value: number | undefined;
    private readonly _freshness: Freshness;
    private readonly _since_epoch_ms: bigint;

    constructor(value: number | undefined, freshness: Freshness, since_epoch_ms: bigint) {
        this._value = value
        this._freshness = freshness
        this._since_epoch_ms = since_epoch_ms
    }

    public get value(): number | undefined {
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

    public with(overrides: {value?: number | undefined; freshness?: Freshness; since_epoch_ms?: bigint}): ActualF64 {
        return new ActualF64(
            'value' in overrides ? overrides.value! : this._value,
            'freshness' in overrides ? overrides.freshness! : this._freshness,
            'since_epoch_ms' in overrides ? overrides.since_epoch_ms! : this._since_epoch_ms
        );
    }

    public static fromPlain(obj: {value: number | undefined; freshness: Freshness; since_epoch_ms: bigint}): ActualF64 {
        return new ActualF64(
            obj.value,
            obj.freshness,
            obj.since_epoch_ms
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return ActualF64.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return ActualF64.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#ActualF64'
    public baboonTypeIdentifier() {
        return ActualF64.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0"]
    public baboonSameInVersions() {
        return ActualF64.BaboonSameInVersions
    }
    public static binCodec(): ActualF64_UEBACodec {
        return ActualF64_UEBACodec.instance
    }
}

export class ActualF64_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: ActualF64, writer: BaboonBinWriter): unknown {
        if (this !== ActualF64_UEBACodec.lazyInstance.value) {
          return ActualF64_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
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
                BinTools.writeF64(buffer, value.value);
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
                BinTools.writeF64(writer, value.value);
            }
            Freshness_UEBACodec.instance.encode(ctx, value.freshness, writer);
            BinTools.writeI64(writer, value.since_epoch_ms);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): ActualF64 {
        if (this !== ActualF64_UEBACodec .lazyInstance.value) {
            return ActualF64_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 1; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const value = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readF64(reader));
        const freshness = Freshness_UEBACodec.instance.decode(ctx, reader);
        const since_epoch_ms = BinTools.readI64(reader);
        return new ActualF64(
            value,
            freshness,
            since_epoch_ms,
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return ActualF64_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return ActualF64_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#ActualF64'
    public baboonTypeIdentifier() {
        return ActualF64_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new ActualF64_UEBACodec())
    public static get instance(): ActualF64_UEBACodec {
        return ActualF64_UEBACodec.lazyInstance.value
    }
}