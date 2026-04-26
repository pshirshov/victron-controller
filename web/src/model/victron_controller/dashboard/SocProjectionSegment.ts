// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'
import {SocProjectionKind, SocProjectionKind_UEBACodec} from './SocProjectionKind'

export class SocProjectionSegment implements BaboonGeneratedLatest {
    private readonly _start_epoch_ms: bigint;
    private readonly _end_epoch_ms: bigint;
    private readonly _start_soc_pct: number;
    private readonly _end_soc_pct: number;
    private readonly _kind: SocProjectionKind;

    constructor(start_epoch_ms: bigint, end_epoch_ms: bigint, start_soc_pct: number, end_soc_pct: number, kind: SocProjectionKind) {
        this._start_epoch_ms = start_epoch_ms
        this._end_epoch_ms = end_epoch_ms
        this._start_soc_pct = start_soc_pct
        this._end_soc_pct = end_soc_pct
        this._kind = kind
    }

    public get start_epoch_ms(): bigint {
        return this._start_epoch_ms;
    }
    public get end_epoch_ms(): bigint {
        return this._end_epoch_ms;
    }
    public get start_soc_pct(): number {
        return this._start_soc_pct;
    }
    public get end_soc_pct(): number {
        return this._end_soc_pct;
    }
    public get kind(): SocProjectionKind {
        return this._kind;
    }

    public toJSON(): Record<string, unknown> {
        return {
            start_epoch_ms: this._start_epoch_ms,
            end_epoch_ms: this._end_epoch_ms,
            start_soc_pct: this._start_soc_pct,
            end_soc_pct: this._end_soc_pct,
            kind: this._kind
        };
    }

    public with(overrides: {start_epoch_ms?: bigint; end_epoch_ms?: bigint; start_soc_pct?: number; end_soc_pct?: number; kind?: SocProjectionKind}): SocProjectionSegment {
        return new SocProjectionSegment(
            'start_epoch_ms' in overrides ? overrides.start_epoch_ms! : this._start_epoch_ms,
            'end_epoch_ms' in overrides ? overrides.end_epoch_ms! : this._end_epoch_ms,
            'start_soc_pct' in overrides ? overrides.start_soc_pct! : this._start_soc_pct,
            'end_soc_pct' in overrides ? overrides.end_soc_pct! : this._end_soc_pct,
            'kind' in overrides ? overrides.kind! : this._kind
        );
    }

    public static fromPlain(obj: {start_epoch_ms: bigint; end_epoch_ms: bigint; start_soc_pct: number; end_soc_pct: number; kind: SocProjectionKind}): SocProjectionSegment {
        return new SocProjectionSegment(
            obj.start_epoch_ms,
            obj.end_epoch_ms,
            obj.start_soc_pct,
            obj.end_soc_pct,
            obj.kind
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return SocProjectionSegment.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return SocProjectionSegment.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#SocProjectionSegment'
    public baboonTypeIdentifier() {
        return SocProjectionSegment.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.2.0", "0.3.0"]
    public baboonSameInVersions() {
        return SocProjectionSegment.BaboonSameInVersions
    }
    public static binCodec(): SocProjectionSegment_UEBACodec {
        return SocProjectionSegment_UEBACodec.instance
    }
}

export class SocProjectionSegment_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: SocProjectionSegment, writer: BaboonBinWriter): unknown {
        if (this !== SocProjectionSegment_UEBACodec.lazyInstance.value) {
          return SocProjectionSegment_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            BinTools.writeI64(buffer, value.start_epoch_ms);
            BinTools.writeI64(buffer, value.end_epoch_ms);
            BinTools.writeF64(buffer, value.start_soc_pct);
            BinTools.writeF64(buffer, value.end_soc_pct);
            SocProjectionKind_UEBACodec.instance.encode(ctx, value.kind, buffer);
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeI64(writer, value.start_epoch_ms);
            BinTools.writeI64(writer, value.end_epoch_ms);
            BinTools.writeF64(writer, value.start_soc_pct);
            BinTools.writeF64(writer, value.end_soc_pct);
            SocProjectionKind_UEBACodec.instance.encode(ctx, value.kind, writer);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): SocProjectionSegment {
        if (this !== SocProjectionSegment_UEBACodec .lazyInstance.value) {
            return SocProjectionSegment_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 0; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const start_epoch_ms = BinTools.readI64(reader);
        const end_epoch_ms = BinTools.readI64(reader);
        const start_soc_pct = BinTools.readF64(reader);
        const end_soc_pct = BinTools.readF64(reader);
        const kind = SocProjectionKind_UEBACodec.instance.decode(ctx, reader);
        return new SocProjectionSegment(
            start_epoch_ms,
            end_epoch_ms,
            start_soc_pct,
            end_soc_pct,
            kind,
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return SocProjectionSegment_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return SocProjectionSegment_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#SocProjectionSegment'
    public baboonTypeIdentifier() {
        return SocProjectionSegment_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new SocProjectionSegment_UEBACodec())
    public static get instance(): SocProjectionSegment_UEBACodec {
        return SocProjectionSegment_UEBACodec.lazyInstance.value
    }
}