// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'
import {TypedSensorEnum, TypedSensorEnum_UEBACodec} from './TypedSensorEnum'
import {TypedSensorZappi, TypedSensorZappi_UEBACodec} from './TypedSensorZappi'

export class TypedSensors implements BaboonGeneratedLatest {
    private readonly _eddi_mode: TypedSensorEnum;
    private readonly _zappi: TypedSensorZappi;

    constructor(eddi_mode: TypedSensorEnum, zappi: TypedSensorZappi) {
        this._eddi_mode = eddi_mode
        this._zappi = zappi
    }

    public get eddi_mode(): TypedSensorEnum {
        return this._eddi_mode;
    }
    public get zappi(): TypedSensorZappi {
        return this._zappi;
    }

    public toJSON(): Record<string, unknown> {
        return {
            eddi_mode: this._eddi_mode,
            zappi: this._zappi
        };
    }

    public with(overrides: {eddi_mode?: TypedSensorEnum; zappi?: TypedSensorZappi}): TypedSensors {
        return new TypedSensors(
            'eddi_mode' in overrides ? overrides.eddi_mode! : this._eddi_mode,
            'zappi' in overrides ? overrides.zappi! : this._zappi
        );
    }

    public static fromPlain(obj: {eddi_mode: TypedSensorEnum; zappi: TypedSensorZappi}): TypedSensors {
        return new TypedSensors(
            obj.eddi_mode,
            obj.zappi
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
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            TypedSensorEnum_UEBACodec.instance.encode(ctx, value.eddi_mode, writer);
            TypedSensorZappi_UEBACodec.instance.encode(ctx, value.zappi, writer);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): TypedSensors {
        if (this !== TypedSensors_UEBACodec .lazyInstance.value) {
            return TypedSensors_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 2; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const eddi_mode = TypedSensorEnum_UEBACodec.instance.decode(ctx, reader);
        const zappi = TypedSensorZappi_UEBACodec.instance.decode(ctx, reader);
        return new TypedSensors(
            eddi_mode,
            zappi,
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