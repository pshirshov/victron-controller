// @ts-nocheck
import {BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../../BaboonSharedRuntime'

export enum BookkeepingKey {
    NextFullCharge = "NextFullCharge",
    AboveSocDate = "AboveSocDate",
    PrevEssState = "PrevEssState"
}

export const BookkeepingKey_values: ReadonlyArray<BookkeepingKey> = [
    BookkeepingKey.NextFullCharge,
    BookkeepingKey.AboveSocDate,
    BookkeepingKey.PrevEssState
] as const;

export function BookkeepingKey_parse(s: string): BookkeepingKey {
    const found = BookkeepingKey_values.find(v => v === s);
    if (found === undefined) {
        throw new Error("Unknown BookkeepingKey variant: " + s);
    }
    return found;
}

/** @deprecated Version 0.2.0 is deprecated, you should migrate to 0.3.0 */
export class BookkeepingKey_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: BookkeepingKey, writer: BaboonBinWriter): unknown {
        if (this !== BookkeepingKey_UEBACodec.lazyInstance.value) {
          return BookkeepingKey_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        switch (value) {
            case "NextFullCharge": BinTools.writeByte(writer, 0); break;
                case "AboveSocDate": BinTools.writeByte(writer, 1); break;
                case "PrevEssState": BinTools.writeByte(writer, 2); break;
            default: throw new Error("Unknown enum variant: " + value);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): BookkeepingKey {
        if (this !== BookkeepingKey_UEBACodec .lazyInstance.value) {
            return BookkeepingKey_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const tag = BinTools.readByte(reader);
        switch (tag) {
            case 0: return "NextFullCharge" as BookkeepingKey;
                case 1: return "AboveSocDate" as BookkeepingKey;
                case 2: return "PrevEssState" as BookkeepingKey;
            default: throw new Error("Unknown enum variant tag: " + tag);
        }
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return BookkeepingKey_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return BookkeepingKey_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#BookkeepingKey'
    public baboonTypeIdentifier() {
        return BookkeepingKey_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new BookkeepingKey_UEBACodec())
    public static get instance(): BookkeepingKey_UEBACodec {
        return BookkeepingKey_UEBACodec.lazyInstance.value
    }
}