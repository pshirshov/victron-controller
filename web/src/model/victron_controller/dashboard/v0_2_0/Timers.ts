// @ts-nocheck
import {BaboonGenerated, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../../BaboonSharedRuntime'
import {Timer, Timer_UEBACodec} from './Timer'

export class Timers implements BaboonGenerated {
    private readonly _entries: Array<Timer>;

    constructor(entries: Array<Timer>) {
        this._entries = entries
    }

    public get entries(): Array<Timer> {
        return this._entries;
    }

    public toJSON(): Record<string, unknown> {
        return {
            entries: this._entries
        };
    }

    public with(overrides: {entries?: Array<Timer>}): Timers {
        return new Timers(
            'entries' in overrides ? overrides.entries! : this._entries
        );
    }

    public static fromPlain(obj: {entries: Array<Timer>}): Timers {
        return new Timers(
            obj.entries
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return Timers.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Timers.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#Timers'
    public baboonTypeIdentifier() {
        return Timers.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.2.0", "0.3.0"]
    public baboonSameInVersions() {
        return Timers.BaboonSameInVersions
    }
    public static binCodec(): Timers_UEBACodec {
        return Timers_UEBACodec.instance
    }
}

/** @deprecated Version 0.2.0 is deprecated, you should migrate to 0.3.0 */
export class Timers_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: Timers, writer: BaboonBinWriter): unknown {
        if (this !== Timers_UEBACodec.lazyInstance.value) {
          return Timers_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeI32(buffer, Array.from(value.entries).length);
            for (const item of value.entries) {
                Timer_UEBACodec.instance.encode(ctx, item, buffer);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeI32(writer, Array.from(value.entries).length);
            for (const item of value.entries) {
                Timer_UEBACodec.instance.encode(ctx, item, writer);
            }
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): Timers {
        if (this !== Timers_UEBACodec .lazyInstance.value) {
            return Timers_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 1; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const entries = Array.from({ length: BinTools.readI32(reader) }, () => Timer_UEBACodec.instance.decode(ctx, reader));
        return new Timers(
            entries,
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return Timers_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Timers_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#Timers'
    public baboonTypeIdentifier() {
        return Timers_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new Timers_UEBACodec())
    public static get instance(): Timers_UEBACodec {
        return Timers_UEBACodec.lazyInstance.value
    }
}