// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'
import {WeatherSocDay, WeatherSocDay_UEBACodec} from './WeatherSocDay'
import {WeatherSocTemperatureSource, WeatherSocTemperatureSource_UEBACodec} from './WeatherSocTemperatureSource'

export class WeatherSocInputs implements BaboonGeneratedLatest {
    private readonly _temperature_c: number;
    private readonly _temperature_source: WeatherSocTemperatureSource;
    private readonly _energy_kwh: number;
    private readonly _day: WeatherSocDay;

    constructor(temperature_c: number, temperature_source: WeatherSocTemperatureSource, energy_kwh: number, day: WeatherSocDay) {
        this._temperature_c = temperature_c
        this._temperature_source = temperature_source
        this._energy_kwh = energy_kwh
        this._day = day
    }

    public get temperature_c(): number {
        return this._temperature_c;
    }
    public get temperature_source(): WeatherSocTemperatureSource {
        return this._temperature_source;
    }
    public get energy_kwh(): number {
        return this._energy_kwh;
    }
    public get day(): WeatherSocDay {
        return this._day;
    }

    public toJSON(): Record<string, unknown> {
        return {
            temperature_c: this._temperature_c,
            temperature_source: this._temperature_source,
            energy_kwh: this._energy_kwh,
            day: this._day
        };
    }

    public with(overrides: {temperature_c?: number; temperature_source?: WeatherSocTemperatureSource; energy_kwh?: number; day?: WeatherSocDay}): WeatherSocInputs {
        return new WeatherSocInputs(
            'temperature_c' in overrides ? overrides.temperature_c! : this._temperature_c,
            'temperature_source' in overrides ? overrides.temperature_source! : this._temperature_source,
            'energy_kwh' in overrides ? overrides.energy_kwh! : this._energy_kwh,
            'day' in overrides ? overrides.day! : this._day
        );
    }

    public static fromPlain(obj: {temperature_c: number; temperature_source: WeatherSocTemperatureSource; energy_kwh: number; day: WeatherSocDay}): WeatherSocInputs {
        return new WeatherSocInputs(
            obj.temperature_c,
            obj.temperature_source,
            obj.energy_kwh,
            obj.day
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return WeatherSocInputs.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return WeatherSocInputs.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#WeatherSocInputs'
    public baboonTypeIdentifier() {
        return WeatherSocInputs.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.3.0"]
    public baboonSameInVersions() {
        return WeatherSocInputs.BaboonSameInVersions
    }
    public static binCodec(): WeatherSocInputs_UEBACodec {
        return WeatherSocInputs_UEBACodec.instance
    }
}

export class WeatherSocInputs_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: WeatherSocInputs, writer: BaboonBinWriter): unknown {
        if (this !== WeatherSocInputs_UEBACodec.lazyInstance.value) {
          return WeatherSocInputs_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            BinTools.writeF64(buffer, value.temperature_c);
            WeatherSocTemperatureSource_UEBACodec.instance.encode(ctx, value.temperature_source, buffer);
            BinTools.writeF64(buffer, value.energy_kwh);
            WeatherSocDay_UEBACodec.instance.encode(ctx, value.day, buffer);
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeF64(writer, value.temperature_c);
            WeatherSocTemperatureSource_UEBACodec.instance.encode(ctx, value.temperature_source, writer);
            BinTools.writeF64(writer, value.energy_kwh);
            WeatherSocDay_UEBACodec.instance.encode(ctx, value.day, writer);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): WeatherSocInputs {
        if (this !== WeatherSocInputs_UEBACodec .lazyInstance.value) {
            return WeatherSocInputs_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 0; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const temperature_c = BinTools.readF64(reader);
        const temperature_source = WeatherSocTemperatureSource_UEBACodec.instance.decode(ctx, reader);
        const energy_kwh = BinTools.readF64(reader);
        const day = WeatherSocDay_UEBACodec.instance.decode(ctx, reader);
        return new WeatherSocInputs(
            temperature_c,
            temperature_source,
            energy_kwh,
            day,
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return WeatherSocInputs_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return WeatherSocInputs_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#WeatherSocInputs'
    public baboonTypeIdentifier() {
        return WeatherSocInputs_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new WeatherSocInputs_UEBACodec())
    public static get instance(): WeatherSocInputs_UEBACodec {
        return WeatherSocInputs_UEBACodec.lazyInstance.value
    }
}