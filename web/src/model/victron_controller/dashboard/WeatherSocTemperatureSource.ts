// @ts-nocheck
import {BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'

export enum WeatherSocTemperatureSource {
    Forecast = "Forecast",
    Sensor = "Sensor"
}

export const WeatherSocTemperatureSource_values: ReadonlyArray<WeatherSocTemperatureSource> = [
    WeatherSocTemperatureSource.Forecast,
    WeatherSocTemperatureSource.Sensor
] as const;

export function WeatherSocTemperatureSource_parse(s: string): WeatherSocTemperatureSource {
    const found = WeatherSocTemperatureSource_values.find(v => v === s);
    if (found === undefined) {
        throw new Error("Unknown WeatherSocTemperatureSource variant: " + s);
    }
    return found;
}

export class WeatherSocTemperatureSource_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: WeatherSocTemperatureSource, writer: BaboonBinWriter): unknown {
        if (this !== WeatherSocTemperatureSource_UEBACodec.lazyInstance.value) {
          return WeatherSocTemperatureSource_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        switch (value) {
            case "Forecast": BinTools.writeByte(writer, 0); break;
                case "Sensor": BinTools.writeByte(writer, 1); break;
            default: throw new Error("Unknown enum variant: " + value);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): WeatherSocTemperatureSource {
        if (this !== WeatherSocTemperatureSource_UEBACodec .lazyInstance.value) {
            return WeatherSocTemperatureSource_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const tag = BinTools.readByte(reader);
        switch (tag) {
            case 0: return "Forecast" as WeatherSocTemperatureSource;
                case 1: return "Sensor" as WeatherSocTemperatureSource;
            default: throw new Error("Unknown enum variant tag: " + tag);
        }
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return WeatherSocTemperatureSource_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return WeatherSocTemperatureSource_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#WeatherSocTemperatureSource'
    public baboonTypeIdentifier() {
        return WeatherSocTemperatureSource_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new WeatherSocTemperatureSource_UEBACodec())
    public static get instance(): WeatherSocTemperatureSource_UEBACodec {
        return WeatherSocTemperatureSource_UEBACodec.lazyInstance.value
    }
}