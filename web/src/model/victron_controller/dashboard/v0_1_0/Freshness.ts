// @ts-nocheck
import {BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../../BaboonSharedRuntime'

export enum Freshness {
    Unknown = "Unknown",
    Fresh = "Fresh",
    Stale = "Stale",
    Deprecated = "Deprecated"
}

export const Freshness_values: ReadonlyArray<Freshness> = [
    Freshness.Unknown,
    Freshness.Fresh,
    Freshness.Stale,
    Freshness.Deprecated
] as const;

export function Freshness_parse(s: string): Freshness {
    const found = Freshness_values.find(v => v === s);
    if (found === undefined) {
        throw new Error("Unknown Freshness variant: " + s);
    }
    return found;
}

/** @deprecated Version 0.1.0 is deprecated, you should migrate to 0.3.0 */
export class Freshness_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: Freshness, writer: BaboonBinWriter): unknown {
        if (this !== Freshness_UEBACodec.lazyInstance.value) {
          return Freshness_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        switch (value) {
            case "Unknown": BinTools.writeByte(writer, 0); break;
                case "Fresh": BinTools.writeByte(writer, 1); break;
                case "Stale": BinTools.writeByte(writer, 2); break;
                case "Deprecated": BinTools.writeByte(writer, 3); break;
            default: throw new Error("Unknown enum variant: " + value);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): Freshness {
        if (this !== Freshness_UEBACodec .lazyInstance.value) {
            return Freshness_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const tag = BinTools.readByte(reader);
        switch (tag) {
            case 0: return "Unknown" as Freshness;
                case 1: return "Fresh" as Freshness;
                case 2: return "Stale" as Freshness;
                case 3: return "Deprecated" as Freshness;
            default: throw new Error("Unknown enum variant tag: " + tag);
        }
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return Freshness_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Freshness_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#Freshness'
    public baboonTypeIdentifier() {
        return Freshness_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new Freshness_UEBACodec())
    public static get instance(): Freshness_UEBACodec {
        return Freshness_UEBACodec.lazyInstance.value
    }
}