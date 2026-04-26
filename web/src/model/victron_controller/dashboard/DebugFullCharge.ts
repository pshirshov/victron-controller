// @ts-nocheck
import {BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'

export enum DebugFullCharge {
    Forbid = "Forbid",
    Force = "Force",
    Auto = "Auto"
}

export const DebugFullCharge_values: ReadonlyArray<DebugFullCharge> = [
    DebugFullCharge.Forbid,
    DebugFullCharge.Force,
    DebugFullCharge.Auto
] as const;

export function DebugFullCharge_parse(s: string): DebugFullCharge {
    const found = DebugFullCharge_values.find(v => v === s);
    if (found === undefined) {
        throw new Error("Unknown DebugFullCharge variant: " + s);
    }
    return found;
}

export class DebugFullCharge_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: DebugFullCharge, writer: BaboonBinWriter): unknown {
        if (this !== DebugFullCharge_UEBACodec.lazyInstance.value) {
          return DebugFullCharge_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        switch (value) {
            case "Forbid": BinTools.writeByte(writer, 0); break;
                case "Force": BinTools.writeByte(writer, 1); break;
                case "Auto": BinTools.writeByte(writer, 2); break;
            default: throw new Error("Unknown enum variant: " + value);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): DebugFullCharge {
        if (this !== DebugFullCharge_UEBACodec .lazyInstance.value) {
            return DebugFullCharge_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const tag = BinTools.readByte(reader);
        switch (tag) {
            case 0: return "Forbid" as DebugFullCharge;
                case 1: return "Force" as DebugFullCharge;
                case 2: return "Auto" as DebugFullCharge;
            default: throw new Error("Unknown enum variant tag: " + tag);
        }
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return DebugFullCharge_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return DebugFullCharge_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#DebugFullCharge'
    public baboonTypeIdentifier() {
        return DebugFullCharge_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new DebugFullCharge_UEBACodec())
    public static get instance(): DebugFullCharge_UEBACodec {
        return DebugFullCharge_UEBACodec.lazyInstance.value
    }
}