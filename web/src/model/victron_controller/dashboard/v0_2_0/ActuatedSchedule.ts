// @ts-nocheck
import {BaboonGenerated, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../../BaboonSharedRuntime'
import {Freshness, Freshness_UEBACodec} from './Freshness'
import {TargetPhase, TargetPhase_UEBACodec} from './TargetPhase'
import {Owner, Owner_UEBACodec} from './Owner'
import {ScheduleSpec, ScheduleSpec_UEBACodec} from './ScheduleSpec'

export class ActuatedSchedule implements BaboonGenerated {
    private readonly _target: ScheduleSpec | undefined;
    private readonly _target_owner: Owner;
    private readonly _target_phase: TargetPhase;
    private readonly _target_since_epoch_ms: bigint;
    private readonly _actual: ScheduleSpec | undefined;
    private readonly _actual_freshness: Freshness;
    private readonly _actual_since_epoch_ms: bigint;

    constructor(target: ScheduleSpec | undefined, target_owner: Owner, target_phase: TargetPhase, target_since_epoch_ms: bigint, actual: ScheduleSpec | undefined, actual_freshness: Freshness, actual_since_epoch_ms: bigint) {
        this._target = target
        this._target_owner = target_owner
        this._target_phase = target_phase
        this._target_since_epoch_ms = target_since_epoch_ms
        this._actual = actual
        this._actual_freshness = actual_freshness
        this._actual_since_epoch_ms = actual_since_epoch_ms
    }

    public get target(): ScheduleSpec | undefined {
        return this._target;
    }
    public get target_owner(): Owner {
        return this._target_owner;
    }
    public get target_phase(): TargetPhase {
        return this._target_phase;
    }
    public get target_since_epoch_ms(): bigint {
        return this._target_since_epoch_ms;
    }
    public get actual(): ScheduleSpec | undefined {
        return this._actual;
    }
    public get actual_freshness(): Freshness {
        return this._actual_freshness;
    }
    public get actual_since_epoch_ms(): bigint {
        return this._actual_since_epoch_ms;
    }

    public toJSON(): Record<string, unknown> {
        return {
            target: this._target !== undefined ? this._target : undefined,
            target_owner: this._target_owner,
            target_phase: this._target_phase,
            target_since_epoch_ms: this._target_since_epoch_ms,
            actual: this._actual !== undefined ? this._actual : undefined,
            actual_freshness: this._actual_freshness,
            actual_since_epoch_ms: this._actual_since_epoch_ms
        };
    }

    public with(overrides: {target?: ScheduleSpec | undefined; target_owner?: Owner; target_phase?: TargetPhase; target_since_epoch_ms?: bigint; actual?: ScheduleSpec | undefined; actual_freshness?: Freshness; actual_since_epoch_ms?: bigint}): ActuatedSchedule {
        return new ActuatedSchedule(
            'target' in overrides ? overrides.target! : this._target,
            'target_owner' in overrides ? overrides.target_owner! : this._target_owner,
            'target_phase' in overrides ? overrides.target_phase! : this._target_phase,
            'target_since_epoch_ms' in overrides ? overrides.target_since_epoch_ms! : this._target_since_epoch_ms,
            'actual' in overrides ? overrides.actual! : this._actual,
            'actual_freshness' in overrides ? overrides.actual_freshness! : this._actual_freshness,
            'actual_since_epoch_ms' in overrides ? overrides.actual_since_epoch_ms! : this._actual_since_epoch_ms
        );
    }

    public static fromPlain(obj: {target: ScheduleSpec | undefined; target_owner: Owner; target_phase: TargetPhase; target_since_epoch_ms: bigint; actual: ScheduleSpec | undefined; actual_freshness: Freshness; actual_since_epoch_ms: bigint}): ActuatedSchedule {
        return new ActuatedSchedule(
            obj.target,
            obj.target_owner,
            obj.target_phase,
            obj.target_since_epoch_ms,
            obj.actual,
            obj.actual_freshness,
            obj.actual_since_epoch_ms
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return ActuatedSchedule.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return ActuatedSchedule.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#ActuatedSchedule'
    public baboonTypeIdentifier() {
        return ActuatedSchedule.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0", "0.2.0", "0.3.0"]
    public baboonSameInVersions() {
        return ActuatedSchedule.BaboonSameInVersions
    }
    public static binCodec(): ActuatedSchedule_UEBACodec {
        return ActuatedSchedule_UEBACodec.instance
    }
}

/** @deprecated Version 0.2.0 is deprecated, you should migrate to 0.3.0 */
export class ActuatedSchedule_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: ActuatedSchedule, writer: BaboonBinWriter): unknown {
        if (this !== ActuatedSchedule_UEBACodec.lazyInstance.value) {
          return ActuatedSchedule_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.target === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                ScheduleSpec_UEBACodec.instance.encode(ctx, value.target, buffer);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            Owner_UEBACodec.instance.encode(ctx, value.target_owner, buffer);
            TargetPhase_UEBACodec.instance.encode(ctx, value.target_phase, buffer);
            BinTools.writeI64(buffer, value.target_since_epoch_ms);
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.actual === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                ScheduleSpec_UEBACodec.instance.encode(ctx, value.actual, buffer);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            Freshness_UEBACodec.instance.encode(ctx, value.actual_freshness, buffer);
            BinTools.writeI64(buffer, value.actual_since_epoch_ms);
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            if (value.target === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                ScheduleSpec_UEBACodec.instance.encode(ctx, value.target, writer);
            }
            Owner_UEBACodec.instance.encode(ctx, value.target_owner, writer);
            TargetPhase_UEBACodec.instance.encode(ctx, value.target_phase, writer);
            BinTools.writeI64(writer, value.target_since_epoch_ms);
            if (value.actual === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                ScheduleSpec_UEBACodec.instance.encode(ctx, value.actual, writer);
            }
            Freshness_UEBACodec.instance.encode(ctx, value.actual_freshness, writer);
            BinTools.writeI64(writer, value.actual_since_epoch_ms);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): ActuatedSchedule {
        if (this !== ActuatedSchedule_UEBACodec .lazyInstance.value) {
            return ActuatedSchedule_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 2; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const target = (BinTools.readByte(reader) === 0 ? undefined : ScheduleSpec_UEBACodec.instance.decode(ctx, reader));
        const target_owner = Owner_UEBACodec.instance.decode(ctx, reader);
        const target_phase = TargetPhase_UEBACodec.instance.decode(ctx, reader);
        const target_since_epoch_ms = BinTools.readI64(reader);
        const actual = (BinTools.readByte(reader) === 0 ? undefined : ScheduleSpec_UEBACodec.instance.decode(ctx, reader));
        const actual_freshness = Freshness_UEBACodec.instance.decode(ctx, reader);
        const actual_since_epoch_ms = BinTools.readI64(reader);
        return new ActuatedSchedule(
            target,
            target_owner,
            target_phase,
            target_since_epoch_ms,
            actual,
            actual_freshness,
            actual_since_epoch_ms,
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return ActuatedSchedule_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return ActuatedSchedule_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#ActuatedSchedule'
    public baboonTypeIdentifier() {
        return ActuatedSchedule_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new ActuatedSchedule_UEBACodec())
    public static get instance(): ActuatedSchedule_UEBACodec {
        return ActuatedSchedule_UEBACodec.lazyInstance.value
    }
}