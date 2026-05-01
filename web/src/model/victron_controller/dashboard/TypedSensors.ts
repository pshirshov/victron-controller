// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'
import {TypedSensorString, TypedSensorString_UEBACodec} from './TypedSensorString'
import {TypedSensorEnum, TypedSensorEnum_UEBACodec} from './TypedSensorEnum'
import {TypedSensorZappi, TypedSensorZappi_UEBACodec} from './TypedSensorZappi'

export class TypedSensors implements BaboonGeneratedLatest {
    private readonly _eddi_mode: TypedSensorEnum;
    private readonly _zappi: TypedSensorZappi;
    private readonly _timezone: TypedSensorString;
    private readonly _sunrise: TypedSensorString;
    private readonly _sunset: TypedSensorString;

    constructor(eddi_mode: TypedSensorEnum, zappi: TypedSensorZappi, timezone: TypedSensorString, sunrise: TypedSensorString, sunset: TypedSensorString) {
        this._eddi_mode = eddi_mode
        this._zappi = zappi
        this._timezone = timezone
        this._sunrise = sunrise
        this._sunset = sunset
    }

    public get eddi_mode(): TypedSensorEnum {
        return this._eddi_mode;
    }
    public get zappi(): TypedSensorZappi {
        return this._zappi;
    }
    public get timezone(): TypedSensorString {
        return this._timezone;
    }
    public get sunrise(): TypedSensorString {
        return this._sunrise;
    }
    public get sunset(): TypedSensorString {
        return this._sunset;
    }

    public toJSON(): Record<string, unknown> {
        return {
            eddi_mode: this._eddi_mode,
            zappi: this._zappi,
            timezone: this._timezone,
            sunrise: this._sunrise,
            sunset: this._sunset
        };
    }

    public with(overrides: {eddi_mode?: TypedSensorEnum; zappi?: TypedSensorZappi; timezone?: TypedSensorString; sunrise?: TypedSensorString; sunset?: TypedSensorString}): TypedSensors {
        return new TypedSensors(
            'eddi_mode' in overrides ? overrides.eddi_mode! : this._eddi_mode,
            'zappi' in overrides ? overrides.zappi! : this._zappi,
            'timezone' in overrides ? overrides.timezone! : this._timezone,
            'sunrise' in overrides ? overrides.sunrise! : this._sunrise,
            'sunset' in overrides ? overrides.sunset! : this._sunset
        );
    }

    public static fromPlain(obj: {eddi_mode: TypedSensorEnum; zappi: TypedSensorZappi; timezone: TypedSensorString; sunrise: TypedSensorString; sunset: TypedSensorString}): TypedSensors {
        return new TypedSensors(
            obj.eddi_mode,
            obj.zappi,
            obj.timezone,
            obj.sunrise,
            obj.sunset
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return TypedSensors.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return TypedSensors.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#TypedSensors'
    public baboonTypeIdentifier() {
        return TypedSensors.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.3.0"]
    public baboonSameInVersions() {
        return TypedSensors.BaboonSameInVersions
    }
    public static binCodec(): TypedSensors_UEBACodec {
        return TypedSensors_UEBACodec.instance
    }
}

export class TypedSensors_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: TypedSensors, writer: BaboonBinWriter): unknown {
        if (this !== TypedSensors_UEBACodec.lazyInstance.value) {
          return TypedSensors_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                TypedSensorEnum_UEBACodec.instance.encode(ctx, value.eddi_mode, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                TypedSensorZappi_UEBACodec.instance.encode(ctx, value.zappi, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                TypedSensorString_UEBACodec.instance.encode(ctx, value.timezone, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                TypedSensorString_UEBACodec.instance.encode(ctx, value.sunrise, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                TypedSensorString_UEBACodec.instance.encode(ctx, value.sunset, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            TypedSensorEnum_UEBACodec.instance.encode(ctx, value.eddi_mode, writer);
            TypedSensorZappi_UEBACodec.instance.encode(ctx, value.zappi, writer);
            TypedSensorString_UEBACodec.instance.encode(ctx, value.timezone, writer);
            TypedSensorString_UEBACodec.instance.encode(ctx, value.sunrise, writer);
            TypedSensorString_UEBACodec.instance.encode(ctx, value.sunset, writer);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): TypedSensors {
        if (this !== TypedSensors_UEBACodec .lazyInstance.value) {
            return TypedSensors_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 5; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const eddi_mode = TypedSensorEnum_UEBACodec.instance.decode(ctx, reader);
        const zappi = TypedSensorZappi_UEBACodec.instance.decode(ctx, reader);
        const timezone = TypedSensorString_UEBACodec.instance.decode(ctx, reader);
        const sunrise = TypedSensorString_UEBACodec.instance.decode(ctx, reader);
        const sunset = TypedSensorString_UEBACodec.instance.decode(ctx, reader);
        return new TypedSensors(
            eddi_mode,
            zappi,
            timezone,
            sunrise,
            sunset,
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return TypedSensors_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return TypedSensors_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#TypedSensors'
    public baboonTypeIdentifier() {
        return TypedSensors_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new TypedSensors_UEBACodec())
    public static get instance(): TypedSensors_UEBACodec {
        return TypedSensors_UEBACodec.lazyInstance.value
    }
}