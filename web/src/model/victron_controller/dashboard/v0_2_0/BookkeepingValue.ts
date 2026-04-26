// @ts-nocheck
import {BaboonGenerated, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../../BaboonSharedRuntime'

export type BookkeepingValue = NaiveDateTime | Cleared

export const BookkeepingValue = {
    BaboonDomainVersion: '0.2.0',
    BaboonDomainIdentifier: 'victron_controller.dashboard',
    BaboonTypeIdentifier: 'victron_controller.dashboard/:#BookkeepingValue',
    BaboonSameInVersions: ["0.2.0", "0.3.0"],
    BaboonAdtTypeIdentifier: 'victron_controller.dashboard/:#BookkeepingValue',
    binCodec(): BookkeepingValue_UEBACodec {
        return BookkeepingValue_UEBACodec.instance
    }
} as const

export function isNaiveDateTime(value: BookkeepingValue): value is NaiveDateTime { return value instanceof NaiveDateTime; }
export function isCleared(value: BookkeepingValue): value is Cleared { return value instanceof Cleared; }

export class NaiveDateTime implements BaboonGenerated {
    private readonly _iso: string;

    constructor(iso: string) {
        this._iso = iso
    }

    public get iso(): string {
        return this._iso;
    }

    public toJSON(): Record<string, unknown> {
        return {
            iso: this._iso
        };
    }

    public with(overrides: {iso?: string}): NaiveDateTime {
        return new NaiveDateTime(
            'iso' in overrides ? overrides.iso! : this._iso
        );
    }

    public static fromPlain(obj: {iso: string}): NaiveDateTime {
        return new NaiveDateTime(
            obj.iso
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return NaiveDateTime.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return NaiveDateTime.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#BookkeepingValue]#NaiveDateTime'
    public baboonTypeIdentifier() {
        return NaiveDateTime.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.2.0", "0.3.0"]
    public baboonSameInVersions() {
        return NaiveDateTime.BaboonSameInVersions
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#BookkeepingValue]#NaiveDateTime'
    public baboonAdtTypeIdentifier() {
        return NaiveDateTime.BaboonAdtTypeIdentifier
    }
    
    public static binCodec(): NaiveDateTime_UEBACodec {
        return NaiveDateTime_UEBACodec.instance
    }
}

/** @deprecated Version 0.2.0 is deprecated, you should migrate to 0.3.0 */
export class NaiveDateTime_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: NaiveDateTime, writer: BaboonBinWriter): unknown {
        if (this !== NaiveDateTime_UEBACodec.lazyInstance.value) {
          return NaiveDateTime_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeString(buffer, value.iso);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeString(writer, value.iso);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): NaiveDateTime {
        if (this !== NaiveDateTime_UEBACodec .lazyInstance.value) {
            return NaiveDateTime_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 1; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const iso = BinTools.readString(reader);
        return new NaiveDateTime(
            iso,
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return NaiveDateTime_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return NaiveDateTime_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#BookkeepingValue]#NaiveDateTime'
    public baboonTypeIdentifier() {
        return NaiveDateTime_UEBACodec.BaboonTypeIdentifier
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#BookkeepingValue]#NaiveDateTime'
    public baboonAdtTypeIdentifier() {
        return NaiveDateTime_UEBACodec.BaboonAdtTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new NaiveDateTime_UEBACodec())
    public static get instance(): NaiveDateTime_UEBACodec {
        return NaiveDateTime_UEBACodec.lazyInstance.value
    }
}

export class Cleared implements BaboonGenerated {
    

    constructor() {
        
    }

    

    public toJSON(): Record<string, unknown> {
        return {
            
        };
    }

    public with(overrides: {}): Cleared {
        return new Cleared(
            
        );
    }

    public static fromPlain(obj: {}): Cleared {
        return new Cleared(
            
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return Cleared.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Cleared.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#BookkeepingValue]#Cleared'
    public baboonTypeIdentifier() {
        return Cleared.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.2.0", "0.3.0"]
    public baboonSameInVersions() {
        return Cleared.BaboonSameInVersions
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#BookkeepingValue]#Cleared'
    public baboonAdtTypeIdentifier() {
        return Cleared.BaboonAdtTypeIdentifier
    }
    
    public static binCodec(): Cleared_UEBACodec {
        return Cleared_UEBACodec.instance
    }
}

/** @deprecated Version 0.2.0 is deprecated, you should migrate to 0.3.0 */
export class Cleared_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: Cleared, writer: BaboonBinWriter): unknown {
        if (this !== Cleared_UEBACodec.lazyInstance.value) {
          return Cleared_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): Cleared {
        if (this !== Cleared_UEBACodec .lazyInstance.value) {
            return Cleared_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 0; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        
        return new Cleared(
            
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return Cleared_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Cleared_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#BookkeepingValue]#Cleared'
    public baboonTypeIdentifier() {
        return Cleared_UEBACodec.BaboonTypeIdentifier
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/[victron_controller.dashboard/:#BookkeepingValue]#Cleared'
    public baboonAdtTypeIdentifier() {
        return Cleared_UEBACodec.BaboonAdtTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new Cleared_UEBACodec())
    public static get instance(): Cleared_UEBACodec {
        return Cleared_UEBACodec.lazyInstance.value
    }
}


/** @deprecated Version 0.2.0 is deprecated, you should migrate to 0.3.0 */
export class BookkeepingValue_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: BookkeepingValue, writer: BaboonBinWriter): unknown {
        if (this !== BookkeepingValue_UEBACodec.lazyInstance.value) {
          return BookkeepingValue_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (value instanceof NaiveDateTime) {
                BinTools.writeByte(writer, 0);
                NaiveDateTime_UEBACodec.instance.encode(ctx, value, writer);
            }
            if (value instanceof Cleared) {
                BinTools.writeByte(writer, 1);
                Cleared_UEBACodec.instance.encode(ctx, value, writer);
            }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): BookkeepingValue {
        if (this !== BookkeepingValue_UEBACodec .lazyInstance.value) {
            return BookkeepingValue_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const tag = BinTools.readByte(reader);
        switch (tag) {
            case 0: return NaiveDateTime_UEBACodec.instance.decode(ctx, reader)
                case 1: return Cleared_UEBACodec.instance.decode(ctx, reader)
            default: throw new Error("Unknown ADT branch tag: " + tag);
        }
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return BookkeepingValue_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return BookkeepingValue_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#BookkeepingValue'
    public baboonTypeIdentifier() {
        return BookkeepingValue_UEBACodec.BaboonTypeIdentifier
    }
    public static readonly BaboonAdtTypeIdentifier = 'victron_controller.dashboard/:#BookkeepingValue'
    public baboonAdtTypeIdentifier() {
        return BookkeepingValue_UEBACodec.BaboonAdtTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new BookkeepingValue_UEBACodec())
    public static get instance(): BookkeepingValue_UEBACodec {
        return BookkeepingValue_UEBACodec.lazyInstance.value
    }
}