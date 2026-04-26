// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'
import {TargetPhase, TargetPhase_UEBACodec} from './TargetPhase'
import {Owner, Owner_UEBACodec} from './Owner'
import {ActualF64, ActualF64_UEBACodec} from './ActualF64'

export class ActuatedF64 implements BaboonGeneratedLatest {
    private readonly _target_value: number | undefined;
    private readonly _target_owner: Owner;
    private readonly _target_phase: TargetPhase;
    private readonly _target_since_epoch_ms: bigint;
    private readonly _actual: ActualF64;

    constructor(target_value: number | undefined, target_owner: Owner, target_phase: TargetPhase, target_since_epoch_ms: bigint, actual: ActualF64) {
        this._target_value = target_value
        this._target_owner = target_owner
        this._target_phase = target_phase
        this._target_since_epoch_ms = target_since_epoch_ms
        this._actual = actual
    }

    public get target_value(): number | undefined {
        return this._target_value;
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
    public get actual(): ActualF64 {
        return this._actual;
    }

    public toJSON(): Record<string, unknown> {
        return {
            target_value: this._target_value !== undefined ? this._target_value : undefined,
            target_owner: this._target_owner,
            target_phase: this._target_phase,
            target_since_epoch_ms: this._target_since_epoch_ms,
            actual: this._actual
        };
    }

    public with(overrides: {target_value?: number | undefined; target_owner?: Owner; target_phase?: TargetPhase; target_since_epoch_ms?: bigint; actual?: ActualF64}): ActuatedF64 {
        return new ActuatedF64(
            'target_value' in overrides ? overrides.target_value! : this._target_value,
            'target_owner' in overrides ? overrides.target_owner! : this._target_owner,
            'target_phase' in overrides ? overrides.target_phase! : this._target_phase,
            'target_since_epoch_ms' in overrides ? overrides.target_since_epoch_ms! : this._target_since_epoch_ms,
            'actual' in overrides ? overrides.actual! : this._actual
        );
    }

    public static fromPlain(obj: {target_value: number | undefined; target_owner: Owner; target_phase: TargetPhase; target_since_epoch_ms: bigint; actual: ActualF64}): ActuatedF64 {
        return new ActuatedF64(
            obj.target_value,
            obj.target_owner,
            obj.target_phase,
            obj.target_since_epoch_ms,
            obj.actual
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return ActuatedF64.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return ActuatedF64.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#ActuatedF64'
    public baboonTypeIdentifier() {
        return ActuatedF64.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0", "0.2.0", "0.3.0"]
    public baboonSameInVersions() {
        return ActuatedF64.BaboonSameInVersions
    }
    public static binCodec(): ActuatedF64_UEBACodec {
        return ActuatedF64_UEBACodec.instance
    }
}

export class ActuatedF64_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: ActuatedF64, writer: BaboonBinWriter): unknown {
        if (this !== ActuatedF64_UEBACodec.lazyInstance.value) {
          return ActuatedF64_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.target_value === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeF64(buffer, value.target_value);
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
                ActualF64_UEBACodec.instance.encode(ctx, value.actual, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            if (value.target_value === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeF64(writer, value.target_value);
            }
            Owner_UEBACodec.instance.encode(ctx, value.target_owner, writer);
            TargetPhase_UEBACodec.instance.encode(ctx, value.target_phase, writer);
            BinTools.writeI64(writer, value.target_since_epoch_ms);
            ActualF64_UEBACodec.instance.encode(ctx, value.actual, writer);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): ActuatedF64 {
        if (this !== ActuatedF64_UEBACodec .lazyInstance.value) {
            return ActuatedF64_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 2; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const target_value = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readF64(reader));
        const target_owner = Owner_UEBACodec.instance.decode(ctx, reader);
        const target_phase = TargetPhase_UEBACodec.instance.decode(ctx, reader);
        const target_since_epoch_ms = BinTools.readI64(reader);
        const actual = ActualF64_UEBACodec.instance.decode(ctx, reader);
        return new ActuatedF64(
            target_value,
            target_owner,
            target_phase,
            target_since_epoch_ms,
            actual,
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return ActuatedF64_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return ActuatedF64_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#ActuatedF64'
    public baboonTypeIdentifier() {
        return ActuatedF64_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new ActuatedF64_UEBACodec())
    public static get instance(): ActuatedF64_UEBACodec {
        return ActuatedF64_UEBACodec.lazyInstance.value
    }
}