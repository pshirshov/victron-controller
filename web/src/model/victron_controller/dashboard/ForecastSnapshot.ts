// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'

export class ForecastSnapshot implements BaboonGeneratedLatest {
    private readonly _today_kwh: number;
    private readonly _tomorrow_kwh: number;
    private readonly _fetched_at_epoch_ms: bigint;
    private readonly _hourly_kwh: Array<number>;
    private readonly _hourly_temperature_c: Array<number>;

    constructor(today_kwh: number, tomorrow_kwh: number, fetched_at_epoch_ms: bigint, hourly_kwh: Array<number>, hourly_temperature_c: Array<number>) {
        this._today_kwh = today_kwh
        this._tomorrow_kwh = tomorrow_kwh
        this._fetched_at_epoch_ms = fetched_at_epoch_ms
        this._hourly_kwh = hourly_kwh
        this._hourly_temperature_c = hourly_temperature_c
    }

    public get today_kwh(): number {
        return this._today_kwh;
    }
    public get tomorrow_kwh(): number {
        return this._tomorrow_kwh;
    }
    public get fetched_at_epoch_ms(): bigint {
        return this._fetched_at_epoch_ms;
    }
    public get hourly_kwh(): Array<number> {
        return this._hourly_kwh;
    }
    public get hourly_temperature_c(): Array<number> {
        return this._hourly_temperature_c;
    }

    public toJSON(): Record<string, unknown> {
        return {
            today_kwh: this._today_kwh,
            tomorrow_kwh: this._tomorrow_kwh,
            fetched_at_epoch_ms: this._fetched_at_epoch_ms,
            hourly_kwh: this._hourly_kwh,
            hourly_temperature_c: this._hourly_temperature_c
        };
    }

    public with(overrides: {today_kwh?: number; tomorrow_kwh?: number; fetched_at_epoch_ms?: bigint; hourly_kwh?: Array<number>; hourly_temperature_c?: Array<number>}): ForecastSnapshot {
        return new ForecastSnapshot(
            'today_kwh' in overrides ? overrides.today_kwh! : this._today_kwh,
            'tomorrow_kwh' in overrides ? overrides.tomorrow_kwh! : this._tomorrow_kwh,
            'fetched_at_epoch_ms' in overrides ? overrides.fetched_at_epoch_ms! : this._fetched_at_epoch_ms,
            'hourly_kwh' in overrides ? overrides.hourly_kwh! : this._hourly_kwh,
            'hourly_temperature_c' in overrides ? overrides.hourly_temperature_c! : this._hourly_temperature_c
        );
    }

    public static fromPlain(obj: {today_kwh: number; tomorrow_kwh: number; fetched_at_epoch_ms: bigint; hourly_kwh: Array<number>; hourly_temperature_c: Array<number>}): ForecastSnapshot {
        return new ForecastSnapshot(
            obj.today_kwh,
            obj.tomorrow_kwh,
            obj.fetched_at_epoch_ms,
            obj.hourly_kwh,
            obj.hourly_temperature_c
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return ForecastSnapshot.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return ForecastSnapshot.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#ForecastSnapshot'
    public baboonTypeIdentifier() {
        return ForecastSnapshot.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.3.0"]
    public baboonSameInVersions() {
        return ForecastSnapshot.BaboonSameInVersions
    }
    public static binCodec(): ForecastSnapshot_UEBACodec {
        return ForecastSnapshot_UEBACodec.instance
    }
}

export class ForecastSnapshot_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: ForecastSnapshot, writer: BaboonBinWriter): unknown {
        if (this !== ForecastSnapshot_UEBACodec.lazyInstance.value) {
          return ForecastSnapshot_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            BinTools.writeF64(buffer, value.today_kwh);
            BinTools.writeF64(buffer, value.tomorrow_kwh);
            BinTools.writeI64(buffer, value.fetched_at_epoch_ms);
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeI32(buffer, Array.from(value.hourly_kwh).length);
            for (const item of value.hourly_kwh) {
                BinTools.writeF64(buffer, item);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeI32(buffer, Array.from(value.hourly_temperature_c).length);
            for (const item of value.hourly_temperature_c) {
                BinTools.writeF64(buffer, item);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeF64(writer, value.today_kwh);
            BinTools.writeF64(writer, value.tomorrow_kwh);
            BinTools.writeI64(writer, value.fetched_at_epoch_ms);
            BinTools.writeI32(writer, Array.from(value.hourly_kwh).length);
            for (const item of value.hourly_kwh) {
                BinTools.writeF64(writer, item);
            }
            BinTools.writeI32(writer, Array.from(value.hourly_temperature_c).length);
            for (const item of value.hourly_temperature_c) {
                BinTools.writeF64(writer, item);
            }
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): ForecastSnapshot {
        if (this !== ForecastSnapshot_UEBACodec .lazyInstance.value) {
            return ForecastSnapshot_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 2; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const today_kwh = BinTools.readF64(reader);
        const tomorrow_kwh = BinTools.readF64(reader);
        const fetched_at_epoch_ms = BinTools.readI64(reader);
        const hourly_kwh = Array.from({ length: BinTools.readI32(reader) }, () => BinTools.readF64(reader));
        const hourly_temperature_c = Array.from({ length: BinTools.readI32(reader) }, () => BinTools.readF64(reader));
        return new ForecastSnapshot(
            today_kwh,
            tomorrow_kwh,
            fetched_at_epoch_ms,
            hourly_kwh,
            hourly_temperature_c,
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return ForecastSnapshot_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return ForecastSnapshot_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#ForecastSnapshot'
    public baboonTypeIdentifier() {
        return ForecastSnapshot_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new ForecastSnapshot_UEBACodec())
    public static get instance(): ForecastSnapshot_UEBACodec {
        return ForecastSnapshot_UEBACodec.lazyInstance.value
    }
}