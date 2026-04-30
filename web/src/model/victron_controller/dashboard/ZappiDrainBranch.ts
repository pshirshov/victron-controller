// @ts-nocheck
import {BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'

export enum ZappiDrainBranch {
    Tighten = "Tighten",
    Relax = "Relax",
    Bypass = "Bypass",
    Disabled = "Disabled"
}

export const ZappiDrainBranch_values: ReadonlyArray<ZappiDrainBranch> = [
    ZappiDrainBranch.Tighten,
    ZappiDrainBranch.Relax,
    ZappiDrainBranch.Bypass,
    ZappiDrainBranch.Disabled
] as const;

export function ZappiDrainBranch_parse(s: string): ZappiDrainBranch {
    const found = ZappiDrainBranch_values.find(v => v === s);
    if (found === undefined) {
        throw new Error("Unknown ZappiDrainBranch variant: " + s);
    }
    return found;
}

export class ZappiDrainBranch_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: ZappiDrainBranch, writer: BaboonBinWriter): unknown {
        if (this !== ZappiDrainBranch_UEBACodec.lazyInstance.value) {
          return ZappiDrainBranch_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        switch (value) {
            case "Tighten": BinTools.writeByte(writer, 0); break;
                case "Relax": BinTools.writeByte(writer, 1); break;
                case "Bypass": BinTools.writeByte(writer, 2); break;
                case "Disabled": BinTools.writeByte(writer, 3); break;
            default: throw new Error("Unknown enum variant: " + value);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): ZappiDrainBranch {
        if (this !== ZappiDrainBranch_UEBACodec .lazyInstance.value) {
            return ZappiDrainBranch_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const tag = BinTools.readByte(reader);
        switch (tag) {
            case 0: return "Tighten" as ZappiDrainBranch;
                case 1: return "Relax" as ZappiDrainBranch;
                case 2: return "Bypass" as ZappiDrainBranch;
                case 3: return "Disabled" as ZappiDrainBranch;
            default: throw new Error("Unknown enum variant tag: " + tag);
        }
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return ZappiDrainBranch_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return ZappiDrainBranch_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#ZappiDrainBranch'
    public baboonTypeIdentifier() {
        return ZappiDrainBranch_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new ZappiDrainBranch_UEBACodec())
    public static get instance(): ZappiDrainBranch_UEBACodec {
        return ZappiDrainBranch_UEBACodec.lazyInstance.value
    }
}