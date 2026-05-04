// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'
import {WeatherSocCell, WeatherSocCell_UEBACodec} from './WeatherSocCell'

export class WeatherSocTable implements BaboonGeneratedLatest {
    private readonly _very_sunny_warm: WeatherSocCell;
    private readonly _very_sunny_cold: WeatherSocCell;
    private readonly _sunny_warm: WeatherSocCell;
    private readonly _sunny_cold: WeatherSocCell;
    private readonly _mid_warm: WeatherSocCell;
    private readonly _mid_cold: WeatherSocCell;
    private readonly _low_warm: WeatherSocCell;
    private readonly _low_cold: WeatherSocCell;
    private readonly _dim_warm: WeatherSocCell;
    private readonly _dim_cold: WeatherSocCell;
    private readonly _very_dim_warm: WeatherSocCell;
    private readonly _very_dim_cold: WeatherSocCell;

    constructor(very_sunny_warm: WeatherSocCell, very_sunny_cold: WeatherSocCell, sunny_warm: WeatherSocCell, sunny_cold: WeatherSocCell, mid_warm: WeatherSocCell, mid_cold: WeatherSocCell, low_warm: WeatherSocCell, low_cold: WeatherSocCell, dim_warm: WeatherSocCell, dim_cold: WeatherSocCell, very_dim_warm: WeatherSocCell, very_dim_cold: WeatherSocCell) {
        this._very_sunny_warm = very_sunny_warm
        this._very_sunny_cold = very_sunny_cold
        this._sunny_warm = sunny_warm
        this._sunny_cold = sunny_cold
        this._mid_warm = mid_warm
        this._mid_cold = mid_cold
        this._low_warm = low_warm
        this._low_cold = low_cold
        this._dim_warm = dim_warm
        this._dim_cold = dim_cold
        this._very_dim_warm = very_dim_warm
        this._very_dim_cold = very_dim_cold
    }

    public get very_sunny_warm(): WeatherSocCell {
        return this._very_sunny_warm;
    }
    public get very_sunny_cold(): WeatherSocCell {
        return this._very_sunny_cold;
    }
    public get sunny_warm(): WeatherSocCell {
        return this._sunny_warm;
    }
    public get sunny_cold(): WeatherSocCell {
        return this._sunny_cold;
    }
    public get mid_warm(): WeatherSocCell {
        return this._mid_warm;
    }
    public get mid_cold(): WeatherSocCell {
        return this._mid_cold;
    }
    public get low_warm(): WeatherSocCell {
        return this._low_warm;
    }
    public get low_cold(): WeatherSocCell {
        return this._low_cold;
    }
    public get dim_warm(): WeatherSocCell {
        return this._dim_warm;
    }
    public get dim_cold(): WeatherSocCell {
        return this._dim_cold;
    }
    public get very_dim_warm(): WeatherSocCell {
        return this._very_dim_warm;
    }
    public get very_dim_cold(): WeatherSocCell {
        return this._very_dim_cold;
    }

    public toJSON(): Record<string, unknown> {
        return {
            very_sunny_warm: this._very_sunny_warm,
            very_sunny_cold: this._very_sunny_cold,
            sunny_warm: this._sunny_warm,
            sunny_cold: this._sunny_cold,
            mid_warm: this._mid_warm,
            mid_cold: this._mid_cold,
            low_warm: this._low_warm,
            low_cold: this._low_cold,
            dim_warm: this._dim_warm,
            dim_cold: this._dim_cold,
            very_dim_warm: this._very_dim_warm,
            very_dim_cold: this._very_dim_cold
        };
    }

    public with(overrides: {very_sunny_warm?: WeatherSocCell; very_sunny_cold?: WeatherSocCell; sunny_warm?: WeatherSocCell; sunny_cold?: WeatherSocCell; mid_warm?: WeatherSocCell; mid_cold?: WeatherSocCell; low_warm?: WeatherSocCell; low_cold?: WeatherSocCell; dim_warm?: WeatherSocCell; dim_cold?: WeatherSocCell; very_dim_warm?: WeatherSocCell; very_dim_cold?: WeatherSocCell}): WeatherSocTable {
        return new WeatherSocTable(
            'very_sunny_warm' in overrides ? overrides.very_sunny_warm! : this._very_sunny_warm,
            'very_sunny_cold' in overrides ? overrides.very_sunny_cold! : this._very_sunny_cold,
            'sunny_warm' in overrides ? overrides.sunny_warm! : this._sunny_warm,
            'sunny_cold' in overrides ? overrides.sunny_cold! : this._sunny_cold,
            'mid_warm' in overrides ? overrides.mid_warm! : this._mid_warm,
            'mid_cold' in overrides ? overrides.mid_cold! : this._mid_cold,
            'low_warm' in overrides ? overrides.low_warm! : this._low_warm,
            'low_cold' in overrides ? overrides.low_cold! : this._low_cold,
            'dim_warm' in overrides ? overrides.dim_warm! : this._dim_warm,
            'dim_cold' in overrides ? overrides.dim_cold! : this._dim_cold,
            'very_dim_warm' in overrides ? overrides.very_dim_warm! : this._very_dim_warm,
            'very_dim_cold' in overrides ? overrides.very_dim_cold! : this._very_dim_cold
        );
    }

    public static fromPlain(obj: {very_sunny_warm: WeatherSocCell; very_sunny_cold: WeatherSocCell; sunny_warm: WeatherSocCell; sunny_cold: WeatherSocCell; mid_warm: WeatherSocCell; mid_cold: WeatherSocCell; low_warm: WeatherSocCell; low_cold: WeatherSocCell; dim_warm: WeatherSocCell; dim_cold: WeatherSocCell; very_dim_warm: WeatherSocCell; very_dim_cold: WeatherSocCell}): WeatherSocTable {
        return new WeatherSocTable(
            obj.very_sunny_warm,
            obj.very_sunny_cold,
            obj.sunny_warm,
            obj.sunny_cold,
            obj.mid_warm,
            obj.mid_cold,
            obj.low_warm,
            obj.low_cold,
            obj.dim_warm,
            obj.dim_cold,
            obj.very_dim_warm,
            obj.very_dim_cold
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return WeatherSocTable.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return WeatherSocTable.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#WeatherSocTable'
    public baboonTypeIdentifier() {
        return WeatherSocTable.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.3.0"]
    public baboonSameInVersions() {
        return WeatherSocTable.BaboonSameInVersions
    }
    public static binCodec(): WeatherSocTable_UEBACodec {
        return WeatherSocTable_UEBACodec.instance
    }
}

export class WeatherSocTable_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: WeatherSocTable, writer: BaboonBinWriter): unknown {
        if (this !== WeatherSocTable_UEBACodec.lazyInstance.value) {
          return WeatherSocTable_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            WeatherSocCell_UEBACodec.instance.encode(ctx, value.very_sunny_warm, buffer);
            WeatherSocCell_UEBACodec.instance.encode(ctx, value.very_sunny_cold, buffer);
            WeatherSocCell_UEBACodec.instance.encode(ctx, value.sunny_warm, buffer);
            WeatherSocCell_UEBACodec.instance.encode(ctx, value.sunny_cold, buffer);
            WeatherSocCell_UEBACodec.instance.encode(ctx, value.mid_warm, buffer);
            WeatherSocCell_UEBACodec.instance.encode(ctx, value.mid_cold, buffer);
            WeatherSocCell_UEBACodec.instance.encode(ctx, value.low_warm, buffer);
            WeatherSocCell_UEBACodec.instance.encode(ctx, value.low_cold, buffer);
            WeatherSocCell_UEBACodec.instance.encode(ctx, value.dim_warm, buffer);
            WeatherSocCell_UEBACodec.instance.encode(ctx, value.dim_cold, buffer);
            WeatherSocCell_UEBACodec.instance.encode(ctx, value.very_dim_warm, buffer);
            WeatherSocCell_UEBACodec.instance.encode(ctx, value.very_dim_cold, buffer);
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            WeatherSocCell_UEBACodec.instance.encode(ctx, value.very_sunny_warm, writer);
            WeatherSocCell_UEBACodec.instance.encode(ctx, value.very_sunny_cold, writer);
            WeatherSocCell_UEBACodec.instance.encode(ctx, value.sunny_warm, writer);
            WeatherSocCell_UEBACodec.instance.encode(ctx, value.sunny_cold, writer);
            WeatherSocCell_UEBACodec.instance.encode(ctx, value.mid_warm, writer);
            WeatherSocCell_UEBACodec.instance.encode(ctx, value.mid_cold, writer);
            WeatherSocCell_UEBACodec.instance.encode(ctx, value.low_warm, writer);
            WeatherSocCell_UEBACodec.instance.encode(ctx, value.low_cold, writer);
            WeatherSocCell_UEBACodec.instance.encode(ctx, value.dim_warm, writer);
            WeatherSocCell_UEBACodec.instance.encode(ctx, value.dim_cold, writer);
            WeatherSocCell_UEBACodec.instance.encode(ctx, value.very_dim_warm, writer);
            WeatherSocCell_UEBACodec.instance.encode(ctx, value.very_dim_cold, writer);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): WeatherSocTable {
        if (this !== WeatherSocTable_UEBACodec .lazyInstance.value) {
            return WeatherSocTable_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 0; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const very_sunny_warm = WeatherSocCell_UEBACodec.instance.decode(ctx, reader);
        const very_sunny_cold = WeatherSocCell_UEBACodec.instance.decode(ctx, reader);
        const sunny_warm = WeatherSocCell_UEBACodec.instance.decode(ctx, reader);
        const sunny_cold = WeatherSocCell_UEBACodec.instance.decode(ctx, reader);
        const mid_warm = WeatherSocCell_UEBACodec.instance.decode(ctx, reader);
        const mid_cold = WeatherSocCell_UEBACodec.instance.decode(ctx, reader);
        const low_warm = WeatherSocCell_UEBACodec.instance.decode(ctx, reader);
        const low_cold = WeatherSocCell_UEBACodec.instance.decode(ctx, reader);
        const dim_warm = WeatherSocCell_UEBACodec.instance.decode(ctx, reader);
        const dim_cold = WeatherSocCell_UEBACodec.instance.decode(ctx, reader);
        const very_dim_warm = WeatherSocCell_UEBACodec.instance.decode(ctx, reader);
        const very_dim_cold = WeatherSocCell_UEBACodec.instance.decode(ctx, reader);
        return new WeatherSocTable(
            very_sunny_warm,
            very_sunny_cold,
            sunny_warm,
            sunny_cold,
            mid_warm,
            mid_cold,
            low_warm,
            low_cold,
            dim_warm,
            dim_cold,
            very_dim_warm,
            very_dim_cold,
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return WeatherSocTable_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return WeatherSocTable_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#WeatherSocTable'
    public baboonTypeIdentifier() {
        return WeatherSocTable_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new WeatherSocTable_UEBACodec())
    public static get instance(): WeatherSocTable_UEBACodec {
        return WeatherSocTable_UEBACodec.lazyInstance.value
    }
}