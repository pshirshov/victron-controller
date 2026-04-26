// @ts-nocheck
import {BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../../BaboonSharedRuntime'

export enum ChargeBatteryExtendedMode {
    Auto = "Auto",
    Forced = "Forced",
    Disabled = "Disabled"
}

export const ChargeBatteryExtendedMode_values: ReadonlyArray<ChargeBatteryExtendedMode> = [
    ChargeBatteryExtendedMode.Auto,
    ChargeBatteryExtendedMode.Forced,
    ChargeBatteryExtendedMode.Disabled
] as const;

export function ChargeBatteryExtendedMode_parse(s: string): ChargeBatteryExtendedMode {
    const found = ChargeBatteryExtendedMode_values.find(v => v === s);
    if (found === undefined) {
        throw new Error("Unknown ChargeBatteryExtendedMode variant: " + s);
    }
    return found;
}

/** @deprecated Version 0.2.0 is deprecated, you should migrate to 0.3.0 */
export class ChargeBatteryExtendedMode_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: ChargeBatteryExtendedMode, writer: BaboonBinWriter): unknown {
        if (this !== ChargeBatteryExtendedMode_UEBACodec.lazyInstance.value) {
          return ChargeBatteryExtendedMode_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        switch (value) {
            case "Auto": BinTools.writeByte(writer, 0); break;
                case "Forced": BinTools.writeByte(writer, 1); break;
                case "Disabled": BinTools.writeByte(writer, 2); break;
            default: throw new Error("Unknown enum variant: " + value);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): ChargeBatteryExtendedMode {
        if (this !== ChargeBatteryExtendedMode_UEBACodec .lazyInstance.value) {
            return ChargeBatteryExtendedMode_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const tag = BinTools.readByte(reader);
        switch (tag) {
            case 0: return "Auto" as ChargeBatteryExtendedMode;
                case 1: return "Forced" as ChargeBatteryExtendedMode;
                case 2: return "Disabled" as ChargeBatteryExtendedMode;
            default: throw new Error("Unknown enum variant tag: " + tag);
        }
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return ChargeBatteryExtendedMode_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return ChargeBatteryExtendedMode_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#ChargeBatteryExtendedMode'
    public baboonTypeIdentifier() {
        return ChargeBatteryExtendedMode_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new ChargeBatteryExtendedMode_UEBACodec())
    public static get instance(): ChargeBatteryExtendedMode_UEBACodec {
        return ChargeBatteryExtendedMode_UEBACodec.lazyInstance.value
    }
}