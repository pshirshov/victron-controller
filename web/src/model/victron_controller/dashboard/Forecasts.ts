// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'
import {ForecastSnapshot, ForecastSnapshot_UEBACodec} from './ForecastSnapshot'

export class Forecasts implements BaboonGeneratedLatest {
    private readonly _solcast: ForecastSnapshot | undefined;
    private readonly _forecast_solar: ForecastSnapshot | undefined;
    private readonly _open_meteo: ForecastSnapshot | undefined;

    constructor(solcast: ForecastSnapshot | undefined, forecast_solar: ForecastSnapshot | undefined, open_meteo: ForecastSnapshot | undefined) {
        this._solcast = solcast
        this._forecast_solar = forecast_solar
        this._open_meteo = open_meteo
    }

    public get solcast(): ForecastSnapshot | undefined {
        return this._solcast;
    }
    public get forecast_solar(): ForecastSnapshot | undefined {
        return this._forecast_solar;
    }
    public get open_meteo(): ForecastSnapshot | undefined {
        return this._open_meteo;
    }

    public toJSON(): Record<string, unknown> {
        return {
            solcast: this._solcast !== undefined ? this._solcast : undefined,
            forecast_solar: this._forecast_solar !== undefined ? this._forecast_solar : undefined,
            open_meteo: this._open_meteo !== undefined ? this._open_meteo : undefined
        };
    }

    public with(overrides: {solcast?: ForecastSnapshot | undefined; forecast_solar?: ForecastSnapshot | undefined; open_meteo?: ForecastSnapshot | undefined}): Forecasts {
        return new Forecasts(
            'solcast' in overrides ? overrides.solcast! : this._solcast,
            'forecast_solar' in overrides ? overrides.forecast_solar! : this._forecast_solar,
            'open_meteo' in overrides ? overrides.open_meteo! : this._open_meteo
        );
    }

    public static fromPlain(obj: {solcast: ForecastSnapshot | undefined; forecast_solar: ForecastSnapshot | undefined; open_meteo: ForecastSnapshot | undefined}): Forecasts {
        return new Forecasts(
            obj.solcast,
            obj.forecast_solar,
            obj.open_meteo
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return Forecasts.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Forecasts.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#Forecasts'
    public baboonTypeIdentifier() {
        return Forecasts.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.2.0", "0.3.0"]
    public baboonSameInVersions() {
        return Forecasts.BaboonSameInVersions
    }
    public static binCodec(): Forecasts_UEBACodec {
        return Forecasts_UEBACodec.instance
    }
}

export class Forecasts_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: Forecasts, writer: BaboonBinWriter): unknown {
        if (this !== Forecasts_UEBACodec.lazyInstance.value) {
          return Forecasts_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.solcast === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                ForecastSnapshot_UEBACodec.instance.encode(ctx, value.solcast, buffer);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.forecast_solar === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                ForecastSnapshot_UEBACodec.instance.encode(ctx, value.forecast_solar, buffer);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.open_meteo === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                ForecastSnapshot_UEBACodec.instance.encode(ctx, value.open_meteo, buffer);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            if (value.solcast === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                ForecastSnapshot_UEBACodec.instance.encode(ctx, value.solcast, writer);
            }
            if (value.forecast_solar === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                ForecastSnapshot_UEBACodec.instance.encode(ctx, value.forecast_solar, writer);
            }
            if (value.open_meteo === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                ForecastSnapshot_UEBACodec.instance.encode(ctx, value.open_meteo, writer);
            }
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): Forecasts {
        if (this !== Forecasts_UEBACodec .lazyInstance.value) {
            return Forecasts_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 3; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const solcast = (BinTools.readByte(reader) === 0 ? undefined : ForecastSnapshot_UEBACodec.instance.decode(ctx, reader));
        const forecast_solar = (BinTools.readByte(reader) === 0 ? undefined : ForecastSnapshot_UEBACodec.instance.decode(ctx, reader));
        const open_meteo = (BinTools.readByte(reader) === 0 ? undefined : ForecastSnapshot_UEBACodec.instance.decode(ctx, reader));
        return new Forecasts(
            solcast,
            forecast_solar,
            open_meteo,
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return Forecasts_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Forecasts_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#Forecasts'
    public baboonTypeIdentifier() {
        return Forecasts_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new Forecasts_UEBACodec())
    public static get instance(): Forecasts_UEBACodec {
        return Forecasts_UEBACodec.lazyInstance.value
    }
}