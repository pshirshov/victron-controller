// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'
import {ScheduledAction, ScheduledAction_UEBACodec} from './ScheduledAction'

export class ScheduledActions implements BaboonGeneratedLatest {
    private readonly _entries: Array<ScheduledAction>;

    constructor(entries: Array<ScheduledAction>) {
        this._entries = entries
    }

    public get entries(): Array<ScheduledAction> {
        return this._entries;
    }

    public toJSON(): Record<string, unknown> {
        return {
            entries: this._entries
        };
    }

    public with(overrides: {entries?: Array<ScheduledAction>}): ScheduledActions {
        return new ScheduledActions(
            'entries' in overrides ? overrides.entries! : this._entries
        );
    }

    public static fromPlain(obj: {entries: Array<ScheduledAction>}): ScheduledActions {
        return new ScheduledActions(
            obj.entries
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return ScheduledActions.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return ScheduledActions.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#ScheduledActions'
    public baboonTypeIdentifier() {
        return ScheduledActions.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.2.0", "0.3.0"]
    public baboonSameInVersions() {
        return ScheduledActions.BaboonSameInVersions
    }
    public static binCodec(): ScheduledActions_UEBACodec {
        return ScheduledActions_UEBACodec.instance
    }
}

export class ScheduledActions_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: ScheduledActions, writer: BaboonBinWriter): unknown {
        if (this !== ScheduledActions_UEBACodec.lazyInstance.value) {
          return ScheduledActions_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeI32(buffer, Array.from(value.entries).length);
            for (const item of value.entries) {
                ScheduledAction_UEBACodec.instance.encode(ctx, item, buffer);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeI32(writer, Array.from(value.entries).length);
            for (const item of value.entries) {
                ScheduledAction_UEBACodec.instance.encode(ctx, item, writer);
            }
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): ScheduledActions {
        if (this !== ScheduledActions_UEBACodec .lazyInstance.value) {
            return ScheduledActions_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 1; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const entries = Array.from({ length: BinTools.readI32(reader) }, () => ScheduledAction_UEBACodec.instance.decode(ctx, reader));
        return new ScheduledActions(
            entries,
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return ScheduledActions_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return ScheduledActions_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#ScheduledActions'
    public baboonTypeIdentifier() {
        return ScheduledActions_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new ScheduledActions_UEBACodec())
    public static get instance(): ScheduledActions_UEBACodec {
        return ScheduledActions_UEBACodec.lazyInstance.value
    }
}