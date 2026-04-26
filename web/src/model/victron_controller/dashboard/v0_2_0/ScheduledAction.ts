// @ts-nocheck
import {BaboonGenerated, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../../BaboonSharedRuntime'

export class ScheduledAction implements BaboonGenerated {
    private readonly _label: string;
    private readonly _source: string;
    private readonly _next_fire_epoch_ms: bigint;
    private readonly _period_ms: bigint | undefined;

    constructor(label: string, source: string, next_fire_epoch_ms: bigint, period_ms: bigint | undefined) {
        this._label = label
        this._source = source
        this._next_fire_epoch_ms = next_fire_epoch_ms
        this._period_ms = period_ms
    }

    public get label(): string {
        return this._label;
    }
    public get source(): string {
        return this._source;
    }
    public get next_fire_epoch_ms(): bigint {
        return this._next_fire_epoch_ms;
    }
    public get period_ms(): bigint | undefined {
        return this._period_ms;
    }

    public toJSON(): Record<string, unknown> {
        return {
            label: this._label,
            source: this._source,
            next_fire_epoch_ms: this._next_fire_epoch_ms,
            period_ms: this._period_ms !== undefined ? this._period_ms : undefined
        };
    }

    public with(overrides: {label?: string; source?: string; next_fire_epoch_ms?: bigint; period_ms?: bigint | undefined}): ScheduledAction {
        return new ScheduledAction(
            'label' in overrides ? overrides.label! : this._label,
            'source' in overrides ? overrides.source! : this._source,
            'next_fire_epoch_ms' in overrides ? overrides.next_fire_epoch_ms! : this._next_fire_epoch_ms,
            'period_ms' in overrides ? overrides.period_ms! : this._period_ms
        );
    }

    public static fromPlain(obj: {label: string; source: string; next_fire_epoch_ms: bigint; period_ms: bigint | undefined}): ScheduledAction {
        return new ScheduledAction(
            obj.label,
            obj.source,
            obj.next_fire_epoch_ms,
            obj.period_ms
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return ScheduledAction.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return ScheduledAction.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#ScheduledAction'
    public baboonTypeIdentifier() {
        return ScheduledAction.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.2.0", "0.3.0"]
    public baboonSameInVersions() {
        return ScheduledAction.BaboonSameInVersions
    }
    public static binCodec(): ScheduledAction_UEBACodec {
        return ScheduledAction_UEBACodec.instance
    }
}

/** @deprecated Version 0.2.0 is deprecated, you should migrate to 0.3.0 */
export class ScheduledAction_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: ScheduledAction, writer: BaboonBinWriter): unknown {
        if (this !== ScheduledAction_UEBACodec.lazyInstance.value) {
          return ScheduledAction_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeString(buffer, value.label);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeString(buffer, value.source);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            BinTools.writeI64(buffer, value.next_fire_epoch_ms);
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.period_ms === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeI64(buffer, value.period_ms);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeString(writer, value.label);
            BinTools.writeString(writer, value.source);
            BinTools.writeI64(writer, value.next_fire_epoch_ms);
            if (value.period_ms === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeI64(writer, value.period_ms);
            }
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): ScheduledAction {
        if (this !== ScheduledAction_UEBACodec .lazyInstance.value) {
            return ScheduledAction_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 3; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const label = BinTools.readString(reader);
        const source = BinTools.readString(reader);
        const next_fire_epoch_ms = BinTools.readI64(reader);
        const period_ms = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readI64(reader));
        return new ScheduledAction(
            label,
            source,
            next_fire_epoch_ms,
            period_ms,
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return ScheduledAction_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return ScheduledAction_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#ScheduledAction'
    public baboonTypeIdentifier() {
        return ScheduledAction_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new ScheduledAction_UEBACodec())
    public static get instance(): ScheduledAction_UEBACodec {
        return ScheduledAction_UEBACodec.lazyInstance.value
    }
}