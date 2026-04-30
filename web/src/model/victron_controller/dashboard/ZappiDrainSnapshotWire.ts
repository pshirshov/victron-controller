// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'
import {ZappiDrainBranch, ZappiDrainBranch_UEBACodec} from './ZappiDrainBranch'

export class ZappiDrainSnapshotWire implements BaboonGeneratedLatest {
    private readonly _compensated_drain_w: number;
    private readonly _branch: ZappiDrainBranch;
    private readonly _hard_clamp_engaged: boolean;
    private readonly _hard_clamp_excess_w: number;
    private readonly _threshold_w: number;
    private readonly _hard_clamp_w: number;
    private readonly _captured_at_epoch_ms: bigint;

    constructor(compensated_drain_w: number, branch: ZappiDrainBranch, hard_clamp_engaged: boolean, hard_clamp_excess_w: number, threshold_w: number, hard_clamp_w: number, captured_at_epoch_ms: bigint) {
        this._compensated_drain_w = compensated_drain_w
        this._branch = branch
        this._hard_clamp_engaged = hard_clamp_engaged
        this._hard_clamp_excess_w = hard_clamp_excess_w
        this._threshold_w = threshold_w
        this._hard_clamp_w = hard_clamp_w
        this._captured_at_epoch_ms = captured_at_epoch_ms
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
    public get hard_clamp_excess_w(): number {
        return this._hard_clamp_excess_w;
    }
    public get threshold_w(): number {
        return this._threshold_w;
    }
    public get hard_clamp_w(): number {
        return this._hard_clamp_w;
    }
    public get captured_at_epoch_ms(): bigint {
        return this._captured_at_epoch_ms;
    }

    public toJSON(): Record<string, unknown> {
        return {
            compensated_drain_w: this._compensated_drain_w,
            branch: this._branch,
            hard_clamp_engaged: this._hard_clamp_engaged,
            hard_clamp_excess_w: this._hard_clamp_excess_w,
            threshold_w: this._threshold_w,
            hard_clamp_w: this._hard_clamp_w,
            captured_at_epoch_ms: this._captured_at_epoch_ms
        };
    }

    public with(overrides: {compensated_drain_w?: number; branch?: ZappiDrainBranch; hard_clamp_engaged?: boolean; hard_clamp_excess_w?: number; threshold_w?: number; hard_clamp_w?: number; captured_at_epoch_ms?: bigint}): ZappiDrainSnapshotWire {
        return new ZappiDrainSnapshotWire(
            'compensated_drain_w' in overrides ? overrides.compensated_drain_w! : this._compensated_drain_w,
            'branch' in overrides ? overrides.branch! : this._branch,
            'hard_clamp_engaged' in overrides ? overrides.hard_clamp_engaged! : this._hard_clamp_engaged,
            'hard_clamp_excess_w' in overrides ? overrides.hard_clamp_excess_w! : this._hard_clamp_excess_w,
            'threshold_w' in overrides ? overrides.threshold_w! : this._threshold_w,
            'hard_clamp_w' in overrides ? overrides.hard_clamp_w! : this._hard_clamp_w,
            'captured_at_epoch_ms' in overrides ? overrides.captured_at_epoch_ms! : this._captured_at_epoch_ms
        );
    }

    public static fromPlain(obj: {compensated_drain_w: number; branch: ZappiDrainBranch; hard_clamp_engaged: boolean; hard_clamp_excess_w: number; threshold_w: number; hard_clamp_w: number; captured_at_epoch_ms: bigint}): ZappiDrainSnapshotWire {
        return new ZappiDrainSnapshotWire(
            obj.compensated_drain_w,
            obj.branch,
            obj.hard_clamp_engaged,
            obj.hard_clamp_excess_w,
            obj.threshold_w,
            obj.hard_clamp_w,
            obj.captured_at_epoch_ms
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return ZappiDrainSnapshotWire.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return ZappiDrainSnapshotWire.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#ZappiDrainSnapshotWire'
    public baboonTypeIdentifier() {
        return ZappiDrainSnapshotWire.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.3.0"]
    public baboonSameInVersions() {
        return ZappiDrainSnapshotWire.BaboonSameInVersions
    }
    public static binCodec(): ZappiDrainSnapshotWire_UEBACodec {
        return ZappiDrainSnapshotWire_UEBACodec.instance
    }
}

export class ZappiDrainSnapshotWire_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: ZappiDrainSnapshotWire, writer: BaboonBinWriter): unknown {
        if (this !== ZappiDrainSnapshotWire_UEBACodec.lazyInstance.value) {
          return ZappiDrainSnapshotWire_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            BinTools.writeF64(buffer, value.compensated_drain_w);
            ZappiDrainBranch_UEBACodec.instance.encode(ctx, value.branch, buffer);
            BinTools.writeBool(buffer, value.hard_clamp_engaged);
            BinTools.writeF64(buffer, value.hard_clamp_excess_w);
            BinTools.writeI32(buffer, value.threshold_w);
            BinTools.writeI32(buffer, value.hard_clamp_w);
            BinTools.writeI64(buffer, value.captured_at_epoch_ms);
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeF64(writer, value.compensated_drain_w);
            ZappiDrainBranch_UEBACodec.instance.encode(ctx, value.branch, writer);
            BinTools.writeBool(writer, value.hard_clamp_engaged);
            BinTools.writeF64(writer, value.hard_clamp_excess_w);
            BinTools.writeI32(writer, value.threshold_w);
            BinTools.writeI32(writer, value.hard_clamp_w);
            BinTools.writeI64(writer, value.captured_at_epoch_ms);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): ZappiDrainSnapshotWire {
        if (this !== ZappiDrainSnapshotWire_UEBACodec .lazyInstance.value) {
            return ZappiDrainSnapshotWire_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 0; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const compensated_drain_w = BinTools.readF64(reader);
        const branch = ZappiDrainBranch_UEBACodec.instance.decode(ctx, reader);
        const hard_clamp_engaged = BinTools.readBool(reader);
        const hard_clamp_excess_w = BinTools.readF64(reader);
        const threshold_w = BinTools.readI32(reader);
        const hard_clamp_w = BinTools.readI32(reader);
        const captured_at_epoch_ms = BinTools.readI64(reader);
        return new ZappiDrainSnapshotWire(
            compensated_drain_w,
            branch,
            hard_clamp_engaged,
            hard_clamp_excess_w,
            threshold_w,
            hard_clamp_w,
            captured_at_epoch_ms,
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return ZappiDrainSnapshotWire_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return ZappiDrainSnapshotWire_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#ZappiDrainSnapshotWire'
    public baboonTypeIdentifier() {
        return ZappiDrainSnapshotWire_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new ZappiDrainSnapshotWire_UEBACodec())
    public static get instance(): ZappiDrainSnapshotWire_UEBACodec {
        return ZappiDrainSnapshotWire_UEBACodec.lazyInstance.value
    }
}