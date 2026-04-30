// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'
import {ZappiDrainBranch, ZappiDrainBranch_UEBACodec} from './ZappiDrainBranch'

export class ZappiDrainSample implements BaboonGeneratedLatest {
    private readonly _captured_at_epoch_ms: bigint;
    private readonly _compensated_drain_w: number;
    private readonly _branch: ZappiDrainBranch;
    private readonly _hard_clamp_engaged: boolean;

    constructor(captured_at_epoch_ms: bigint, compensated_drain_w: number, branch: ZappiDrainBranch, hard_clamp_engaged: boolean) {
        this._captured_at_epoch_ms = captured_at_epoch_ms
        this._compensated_drain_w = compensated_drain_w
        this._branch = branch
        this._hard_clamp_engaged = hard_clamp_engaged
    }

    public get captured_at_epoch_ms(): bigint {
        return this._captured_at_epoch_ms;
    }
    public get compensated_drain_w(): number {
        return this._compensated_drain_w;
    }
    public get branch(): ZappiDrainBranch {
        return this._branch;
    }
    public get hard_clamp_engaged(): boolean {
        return this._hard_clamp_engaged;
    }

    public toJSON(): Record<string, unknown> {
        return {
            captured_at_epoch_ms: this._captured_at_epoch_ms,
            compensated_drain_w: this._compensated_drain_w,
            branch: this._branch,
            hard_clamp_engaged: this._hard_clamp_engaged
        };
    }

    public with(overrides: {captured_at_epoch_ms?: bigint; compensated_drain_w?: number; branch?: ZappiDrainBranch; hard_clamp_engaged?: boolean}): ZappiDrainSample {
        return new ZappiDrainSample(
            'captured_at_epoch_ms' in overrides ? overrides.captured_at_epoch_ms! : this._captured_at_epoch_ms,
            'compensated_drain_w' in overrides ? overrides.compensated_drain_w! : this._compensated_drain_w,
            'branch' in overrides ? overrides.branch! : this._branch,
            'hard_clamp_engaged' in overrides ? overrides.hard_clamp_engaged! : this._hard_clamp_engaged
        );
    }

    public static fromPlain(obj: {captured_at_epoch_ms: bigint; compensated_drain_w: number; branch: ZappiDrainBranch; hard_clamp_engaged: boolean}): ZappiDrainSample {
        return new ZappiDrainSample(
            obj.captured_at_epoch_ms,
            obj.compensated_drain_w,
            obj.branch,
            obj.hard_clamp_engaged
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return ZappiDrainSample.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return ZappiDrainSample.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#ZappiDrainSample'
    public baboonTypeIdentifier() {
        return ZappiDrainSample.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.3.0"]
    public baboonSameInVersions() {
        return ZappiDrainSample.BaboonSameInVersions
    }
    public static binCodec(): ZappiDrainSample_UEBACodec {
        return ZappiDrainSample_UEBACodec.instance
    }
}

export class ZappiDrainSample_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: ZappiDrainSample, writer: BaboonBinWriter): unknown {
        if (this !== ZappiDrainSample_UEBACodec.lazyInstance.value) {
          return ZappiDrainSample_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            BinTools.writeI64(buffer, value.captured_at_epoch_ms);
            BinTools.writeF64(buffer, value.compensated_drain_w);
            ZappiDrainBranch_UEBACodec.instance.encode(ctx, value.branch, buffer);
            BinTools.writeBool(buffer, value.hard_clamp_engaged);
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeI64(writer, value.captured_at_epoch_ms);
            BinTools.writeF64(writer, value.compensated_drain_w);
            ZappiDrainBranch_UEBACodec.instance.encode(ctx, value.branch, writer);
            BinTools.writeBool(writer, value.hard_clamp_engaged);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): ZappiDrainSample {
        if (this !== ZappiDrainSample_UEBACodec .lazyInstance.value) {
            return ZappiDrainSample_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 0; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const captured_at_epoch_ms = BinTools.readI64(reader);
        const compensated_drain_w = BinTools.readF64(reader);
        const branch = ZappiDrainBranch_UEBACodec.instance.decode(ctx, reader);
        const hard_clamp_engaged = BinTools.readBool(reader);
        return new ZappiDrainSample(
            captured_at_epoch_ms,
            compensated_drain_w,
            branch,
            hard_clamp_engaged,
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return ZappiDrainSample_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return ZappiDrainSample_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#ZappiDrainSample'
    public baboonTypeIdentifier() {
        return ZappiDrainSample_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new ZappiDrainSample_UEBACodec())
    public static get instance(): ZappiDrainSample_UEBACodec {
        return ZappiDrainSample_UEBACodec.lazyInstance.value
    }
}