// @ts-nocheck
import {BaboonGenerated, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../../BaboonSharedRuntime'

export class SocHistorySample implements BaboonGenerated {
    private readonly _epoch_ms: bigint;
    private readonly _soc_pct: number;

    constructor(epoch_ms: bigint, soc_pct: number) {
        this._epoch_ms = epoch_ms
        this._soc_pct = soc_pct
    }

    public get epoch_ms(): bigint {
        return this._epoch_ms;
    }
    public get soc_pct(): number {
        return this._soc_pct;
    }

    public toJSON(): Record<string, unknown> {
        return {
            epoch_ms: this._epoch_ms,
            soc_pct: this._soc_pct
        };
    }

    public with(overrides: {epoch_ms?: bigint; soc_pct?: number}): SocHistorySample {
        return new SocHistorySample(
            'epoch_ms' in overrides ? overrides.epoch_ms! : this._epoch_ms,
            'soc_pct' in overrides ? overrides.soc_pct! : this._soc_pct
        );
    }

    public static fromPlain(obj: {epoch_ms: bigint; soc_pct: number}): SocHistorySample {
        return new SocHistorySample(
            obj.epoch_ms,
            obj.soc_pct
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return SocHistorySample.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return SocHistorySample.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#SocHistorySample'
    public baboonTypeIdentifier() {
        return SocHistorySample.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.2.0", "0.3.0"]
    public baboonSameInVersions() {
        return SocHistorySample.BaboonSameInVersions
    }
    public static binCodec(): SocHistorySample_UEBACodec {
        return SocHistorySample_UEBACodec.instance
    }
}

/** @deprecated Version 0.2.0 is deprecated, you should migrate to 0.3.0 */
export class SocHistorySample_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: SocHistorySample, writer: BaboonBinWriter): unknown {
        if (this !== SocHistorySample_UEBACodec.lazyInstance.value) {
          return SocHistorySample_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            BinTools.writeI64(buffer, value.epoch_ms);
            BinTools.writeF64(buffer, value.soc_pct);
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeI64(writer, value.epoch_ms);
            BinTools.writeF64(writer, value.soc_pct);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): SocHistorySample {
        if (this !== SocHistorySample_UEBACodec .lazyInstance.value) {
            return SocHistorySample_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 0; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const epoch_ms = BinTools.readI64(reader);
        const soc_pct = BinTools.readF64(reader);
        return new SocHistorySample(
            epoch_ms,
            soc_pct,
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return SocHistorySample_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return SocHistorySample_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#SocHistorySample'
    public baboonTypeIdentifier() {
        return SocHistorySample_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new SocHistorySample_UEBACodec())
    public static get instance(): SocHistorySample_UEBACodec {
        return SocHistorySample_UEBACodec.lazyInstance.value
    }
}