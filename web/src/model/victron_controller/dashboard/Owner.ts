import {BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'

export enum Owner {
    Unset = "Unset",
    System = "System",
    Dashboard = "Dashboard",
    HaMqtt = "HaMqtt",
    WeatherSocPlanner = "WeatherSocPlanner",
    SetpointController = "SetpointController",
    CurrentLimitController = "CurrentLimitController",
    ScheduleController = "ScheduleController",
    ZappiController = "ZappiController",
    EddiController = "EddiController",
    FullChargeScheduler = "FullChargeScheduler"
}

export const Owner_values: ReadonlyArray<Owner> = [
    Owner.Unset,
    Owner.System,
    Owner.Dashboard,
    Owner.HaMqtt,
    Owner.WeatherSocPlanner,
    Owner.SetpointController,
    Owner.CurrentLimitController,
    Owner.ScheduleController,
    Owner.ZappiController,
    Owner.EddiController,
    Owner.FullChargeScheduler
] as const;

export function Owner_parse(s: string): Owner {
    const found = Owner_values.find(v => v === s);
    if (found === undefined) {
        throw new Error("Unknown Owner variant: " + s);
    }
    return found;
}

export class Owner_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: Owner, writer: BaboonBinWriter): unknown {
        if (this !== Owner_UEBACodec.lazyInstance.value) {
          return Owner_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        switch (value) {
            case "Unset": BinTools.writeByte(writer, 0); break;
                case "System": BinTools.writeByte(writer, 1); break;
                case "Dashboard": BinTools.writeByte(writer, 2); break;
                case "HaMqtt": BinTools.writeByte(writer, 3); break;
                case "WeatherSocPlanner": BinTools.writeByte(writer, 4); break;
                case "SetpointController": BinTools.writeByte(writer, 5); break;
                case "CurrentLimitController": BinTools.writeByte(writer, 6); break;
                case "ScheduleController": BinTools.writeByte(writer, 7); break;
                case "ZappiController": BinTools.writeByte(writer, 8); break;
                case "EddiController": BinTools.writeByte(writer, 9); break;
                case "FullChargeScheduler": BinTools.writeByte(writer, 10); break;
            default: throw new Error("Unknown enum variant: " + value);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): Owner {
        if (this !== Owner_UEBACodec .lazyInstance.value) {
            return Owner_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const tag = BinTools.readByte(reader);
        switch (tag) {
            case 0: return "Unset" as Owner;
                case 1: return "System" as Owner;
                case 2: return "Dashboard" as Owner;
                case 3: return "HaMqtt" as Owner;
                case 4: return "WeatherSocPlanner" as Owner;
                case 5: return "SetpointController" as Owner;
                case 6: return "CurrentLimitController" as Owner;
                case 7: return "ScheduleController" as Owner;
                case 8: return "ZappiController" as Owner;
                case 9: return "EddiController" as Owner;
                case 10: return "FullChargeScheduler" as Owner;
            default: throw new Error("Unknown enum variant tag: " + tag);
        }
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return Owner_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Owner_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#Owner'
    public baboonTypeIdentifier() {
        return Owner_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new Owner_UEBACodec())
    public static get instance(): Owner_UEBACodec {
        return Owner_UEBACodec.lazyInstance.value
    }
}