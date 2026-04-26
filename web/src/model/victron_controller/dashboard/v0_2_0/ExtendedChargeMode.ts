// @ts-nocheck
import {BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../../BaboonSharedRuntime'

export enum ExtendedChargeMode {
    Auto = "Auto",
    Forced = "Forced",
    Disabled = "Disabled"
}

export const ExtendedChargeMode_values: ReadonlyArray<ExtendedChargeMode> = [
    ExtendedChargeMode.Auto,
    ExtendedChargeMode.Forced,
    ExtendedChargeMode.Disabled
] as const;

export function ExtendedChargeMode_parse(s: string): ExtendedChargeMode {
    const found = ExtendedChargeMode_values.find(v => v === s);
    if (found === undefined) {
        throw new Error("Unknown ExtendedChargeMode variant: " + s);
    }
    return found;
}

/** @deprecated Version 0.2.0 is deprecated, you should migrate to 0.3.0 */
export class ExtendedChargeMode_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: ExtendedChargeMode, writer: BaboonBinWriter): unknown {
        if (this !== ExtendedChargeMode_UEBACodec.lazyInstance.value) {
          return ExtendedChargeMode_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        switch (value) {
            case "Auto": BinTools.writeByte(writer, 0); break;
                case "Forced": BinTools.writeByte(writer, 1); break;
                case "Disabled": BinTools.writeByte(writer, 2); break;
            default: throw new Error("Unknown enum variant: " + value);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): ExtendedChargeMode {
        if (this !== ExtendedChargeMode_UEBACodec .lazyInstance.value) {
            return ExtendedChargeMode_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const tag = BinTools.readByte(reader);
        switch (tag) {
            case 0: return "Auto" as ExtendedChargeMode;
                case 1: return "Forced" as ExtendedChargeMode;
                case 2: return "Disabled" as ExtendedChargeMode;
            default: throw new Error("Unknown enum variant tag: " + tag);
        }
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return ExtendedChargeMode_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return ExtendedChargeMode_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#ExtendedChargeMode'
    public baboonTypeIdentifier() {
        return ExtendedChargeMode_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new ExtendedChargeMode_UEBACodec())
    public static get instance(): ExtendedChargeMode_UEBACodec {
        return ExtendedChargeMode_UEBACodec.lazyInstance.value
    }
}