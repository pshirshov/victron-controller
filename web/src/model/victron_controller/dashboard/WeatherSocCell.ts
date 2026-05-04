// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'

export class WeatherSocCell implements BaboonGeneratedLatest {
    private readonly _export_soc_threshold: number;
    private readonly _battery_soc_target: number;
    private readonly _discharge_soc_target: number;
    private readonly _extended: boolean;

    constructor(export_soc_threshold: number, battery_soc_target: number, discharge_soc_target: number, extended: boolean) {
        this._export_soc_threshold = export_soc_threshold
        this._battery_soc_target = battery_soc_target
        this._discharge_soc_target = discharge_soc_target
        this._extended = extended
    }

    public get export_soc_threshold(): number {
        return this._export_soc_threshold;
    }
    public get battery_soc_target(): number {
        return this._battery_soc_target;
    }
    public get discharge_soc_target(): number {
        return this._discharge_soc_target;
    }
    public get extended(): boolean {
        return this._extended;
    }

    public toJSON(): Record<string, unknown> {
        return {
            export_soc_threshold: this._export_soc_threshold,
            battery_soc_target: this._battery_soc_target,
            discharge_soc_target: this._discharge_soc_target,
            extended: this._extended
        };
    }

    public with(overrides: {export_soc_threshold?: number; battery_soc_target?: number; discharge_soc_target?: number; extended?: boolean}): WeatherSocCell {
        return new WeatherSocCell(
            'export_soc_threshold' in overrides ? overrides.export_soc_threshold! : this._export_soc_threshold,
            'battery_soc_target' in overrides ? overrides.battery_soc_target! : this._battery_soc_target,
            'discharge_soc_target' in overrides ? overrides.discharge_soc_target! : this._discharge_soc_target,
            'extended' in overrides ? overrides.extended! : this._extended
        );
    }

    public static fromPlain(obj: {export_soc_threshold: number; battery_soc_target: number; discharge_soc_target: number; extended: boolean}): WeatherSocCell {
        return new WeatherSocCell(
            obj.export_soc_threshold,
            obj.battery_soc_target,
            obj.discharge_soc_target,
            obj.extended
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return WeatherSocCell.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return WeatherSocCell.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#WeatherSocCell'
    public baboonTypeIdentifier() {
        return WeatherSocCell.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.3.0"]
    public baboonSameInVersions() {
        return WeatherSocCell.BaboonSameInVersions
    }
    public static binCodec(): WeatherSocCell_UEBACodec {
        return WeatherSocCell_UEBACodec.instance
    }
}

export class WeatherSocCell_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: WeatherSocCell, writer: BaboonBinWriter): unknown {
        if (this !== WeatherSocCell_UEBACodec.lazyInstance.value) {
          return WeatherSocCell_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            BinTools.writeF64(buffer, value.export_soc_threshold);
            BinTools.writeF64(buffer, value.battery_soc_target);
            BinTools.writeF64(buffer, value.discharge_soc_target);
            BinTools.writeBool(buffer, value.extended);
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeF64(writer, value.export_soc_threshold);
            BinTools.writeF64(writer, value.battery_soc_target);
            BinTools.writeF64(writer, value.discharge_soc_target);
            BinTools.writeBool(writer, value.extended);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): WeatherSocCell {
        if (this !== WeatherSocCell_UEBACodec .lazyInstance.value) {
            return WeatherSocCell_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 0; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const export_soc_threshold = BinTools.readF64(reader);
        const battery_soc_target = BinTools.readF64(reader);
        const discharge_soc_target = BinTools.readF64(reader);
        const extended = BinTools.readBool(reader);
        return new WeatherSocCell(
            export_soc_threshold,
            battery_soc_target,
            discharge_soc_target,
            extended,
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return WeatherSocCell_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return WeatherSocCell_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#WeatherSocCell'
    public baboonTypeIdentifier() {
        return WeatherSocCell_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new WeatherSocCell_UEBACodec())
    public static get instance(): WeatherSocCell_UEBACodec {
        return WeatherSocCell_UEBACodec.lazyInstance.value
    }
}