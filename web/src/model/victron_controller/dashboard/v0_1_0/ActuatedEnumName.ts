// @ts-nocheck
import {BaboonGenerated, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../../BaboonSharedRuntime'
import {Owner, Owner_UEBACodec} from './Owner'
import {TargetPhase, TargetPhase_UEBACodec} from './TargetPhase'
import {Freshness, Freshness_UEBACodec} from './Freshness'

export class ActuatedEnumName implements BaboonGenerated {
    private readonly _target_value: string | undefined;
    private readonly _target_owner: Owner;
    private readonly _target_phase: TargetPhase;
    private readonly _target_since_epoch_ms: bigint;
    private readonly _actual_value: string | undefined;
    private readonly _actual_freshness: Freshness;
    private readonly _actual_since_epoch_ms: bigint;

    constructor(target_value: string | undefined, target_owner: Owner, target_phase: TargetPhase, target_since_epoch_ms: bigint, actual_value: string | undefined, actual_freshness: Freshness, actual_since_epoch_ms: bigint) {
        this._target_value = target_value
        this._target_owner = target_owner
        this._target_phase = target_phase
        this._target_since_epoch_ms = target_since_epoch_ms
        this._actual_value = actual_value
        this._actual_freshness = actual_freshness
        this._actual_since_epoch_ms = actual_since_epoch_ms
    }

    public get target_value(): string | undefined {
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
    public get actual_value(): string | undefined {
        return this._actual_value;
    }
    public get actual_freshness(): Freshness {
        return this._actual_freshness;
    }
    public get actual_since_epoch_ms(): bigint {
        return this._actual_since_epoch_ms;
    }

    public toJSON(): Record<string, unknown> {
        return {
            target_value: this._target_value !== undefined ? this._target_value : undefined,
            target_owner: this._target_owner,
            target_phase: this._target_phase,
            target_since_epoch_ms: this._target_since_epoch_ms,
            actual_value: this._actual_value !== undefined ? this._actual_value : undefined,
            actual_freshness: this._actual_freshness,
            actual_since_epoch_ms: this._actual_since_epoch_ms
        };
    }

    public with(overrides: {target_value?: string | undefined; target_owner?: Owner; target_phase?: TargetPhase; target_since_epoch_ms?: bigint; actual_value?: string | undefined; actual_freshness?: Freshness; actual_since_epoch_ms?: bigint}): ActuatedEnumName {
        return new ActuatedEnumName(
            'target_value' in overrides ? overrides.target_value! : this._target_value,
            'target_owner' in overrides ? overrides.target_owner! : this._target_owner,
            'target_phase' in overrides ? overrides.target_phase! : this._target_phase,
            'target_since_epoch_ms' in overrides ? overrides.target_since_epoch_ms! : this._target_since_epoch_ms,
            'actual_value' in overrides ? overrides.actual_value! : this._actual_value,
            'actual_freshness' in overrides ? overrides.actual_freshness! : this._actual_freshness,
            'actual_since_epoch_ms' in overrides ? overrides.actual_since_epoch_ms! : this._actual_since_epoch_ms
        );
    }

    public static fromPlain(obj: {target_value: string | undefined; target_owner: Owner; target_phase: TargetPhase; target_since_epoch_ms: bigint; actual_value: string | undefined; actual_freshness: Freshness; actual_since_epoch_ms: bigint}): ActuatedEnumName {
        return new ActuatedEnumName(
            obj.target_value,
            obj.target_owner,
            obj.target_phase,
            obj.target_since_epoch_ms,
            obj.actual_value,
            obj.actual_freshness,
            obj.actual_since_epoch_ms
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return ActuatedEnumName.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return ActuatedEnumName.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#ActuatedEnumName'
    public baboonTypeIdentifier() {
        return ActuatedEnumName.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0", "0.2.0"]
    public baboonSameInVersions() {
        return ActuatedEnumName.BaboonSameInVersions
    }
    public static binCodec(): ActuatedEnumName_UEBACodec {
        return ActuatedEnumName_UEBACodec.instance
    }
}

/** @deprecated Version 0.1.0 is deprecated, you should migrate to 0.2.0 */
export class ActuatedEnumName_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: ActuatedEnumName, writer: BaboonBinWriter): unknown {
        if (this !== ActuatedEnumName_UEBACodec.lazyInstance.value) {
          return ActuatedEnumName_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
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
                BinTools.writeString(buffer, value.target_value);
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
                if (value.actual_value === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeString(buffer, value.actual_value);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            Freshness_UEBACodec.instance.encode(ctx, value.actual_freshness, buffer);
            BinTools.writeI64(buffer, value.actual_since_epoch_ms);
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            if (value.target_value === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeString(writer, value.target_value);
            }
            Owner_UEBACodec.instance.encode(ctx, value.target_owner, writer);
            TargetPhase_UEBACodec.instance.encode(ctx, value.target_phase, writer);
            BinTools.writeI64(writer, value.target_since_epoch_ms);
            if (value.actual_value === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeString(writer, value.actual_value);
            }
            Freshness_UEBACodec.instance.encode(ctx, value.actual_freshness, writer);
            BinTools.writeI64(writer, value.actual_since_epoch_ms);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): ActuatedEnumName {
        if (this !== ActuatedEnumName_UEBACodec .lazyInstance.value) {
            return ActuatedEnumName_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 2; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const target_value = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readString(reader));
        const target_owner = Owner_UEBACodec.instance.decode(ctx, reader);
        const target_phase = TargetPhase_UEBACodec.instance.decode(ctx, reader);
        const target_since_epoch_ms = BinTools.readI64(reader);
        const actual_value = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readString(reader));
        const actual_freshness = Freshness_UEBACodec.instance.decode(ctx, reader);
        const actual_since_epoch_ms = BinTools.readI64(reader);
        return new ActuatedEnumName(
            target_value,
            target_owner,
            target_phase,
            target_since_epoch_ms,
            actual_value,
            actual_freshness,
            actual_since_epoch_ms,
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return ActuatedEnumName_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return ActuatedEnumName_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#ActuatedEnumName'
    public baboonTypeIdentifier() {
        return ActuatedEnumName_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new ActuatedEnumName_UEBACodec())
    public static get instance(): ActuatedEnumName_UEBACodec {
        return ActuatedEnumName_UEBACodec.lazyInstance.value
    }
}