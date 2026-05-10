// @ts-nocheck
import {BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'

export enum WeatherSocDay {
    Today = "Today",
    Tomorrow = "Tomorrow"
}

export const WeatherSocDay_values: ReadonlyArray<WeatherSocDay> = [
    WeatherSocDay.Today,
    WeatherSocDay.Tomorrow
] as const;

export function WeatherSocDay_parse(s: string): WeatherSocDay {
    const found = WeatherSocDay_values.find(v => v === s);
    if (found === undefined) {
        throw new Error("Unknown WeatherSocDay variant: " + s);
    }
    return found;
}

export class WeatherSocDay_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: WeatherSocDay, writer: BaboonBinWriter): unknown {
        if (this !== WeatherSocDay_UEBACodec.lazyInstance.value) {
          return WeatherSocDay_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        switch (value) {
            case "Today": BinTools.writeByte(writer, 0); break;
                case "Tomorrow": BinTools.writeByte(writer, 1); break;
            default: throw new Error("Unknown enum variant: " + value);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): WeatherSocDay {
        if (this !== WeatherSocDay_UEBACodec .lazyInstance.value) {
            return WeatherSocDay_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const tag = BinTools.readByte(reader);
        switch (tag) {
            case 0: return "Today" as WeatherSocDay;
                case 1: return "Tomorrow" as WeatherSocDay;
            default: throw new Error("Unknown enum variant tag: " + tag);
        }
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return WeatherSocDay_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return WeatherSocDay_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#WeatherSocDay'
    public baboonTypeIdentifier() {
        return WeatherSocDay_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new WeatherSocDay_UEBACodec())
    public static get instance(): WeatherSocDay_UEBACodec {
        return WeatherSocDay_UEBACodec.lazyInstance.value
    }
}