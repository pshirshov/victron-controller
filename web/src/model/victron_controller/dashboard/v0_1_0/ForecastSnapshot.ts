// @ts-nocheck
import {BaboonGenerated, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../../BaboonSharedRuntime'

export class ForecastSnapshot implements BaboonGenerated {
    private readonly _today_kwh: number;
    private readonly _tomorrow_kwh: number;
    private readonly _fetched_at_epoch_ms: bigint;

    constructor(today_kwh: number, tomorrow_kwh: number, fetched_at_epoch_ms: bigint) {
        this._today_kwh = today_kwh
        this._tomorrow_kwh = tomorrow_kwh
        this._fetched_at_epoch_ms = fetched_at_epoch_ms
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

    public toJSON(): Record<string, unknown> {
        return {
            today_kwh: this._today_kwh,
            tomorrow_kwh: this._tomorrow_kwh,
            fetched_at_epoch_ms: this._fetched_at_epoch_ms
        };
    }

    public with(overrides: {today_kwh?: number; tomorrow_kwh?: number; fetched_at_epoch_ms?: bigint}): ForecastSnapshot {
        return new ForecastSnapshot(
            'today_kwh' in overrides ? overrides.today_kwh! : this._today_kwh,
            'tomorrow_kwh' in overrides ? overrides.tomorrow_kwh! : this._tomorrow_kwh,
            'fetched_at_epoch_ms' in overrides ? overrides.fetched_at_epoch_ms! : this._fetched_at_epoch_ms
        );
    }

    public static fromPlain(obj: {today_kwh: number; tomorrow_kwh: number; fetched_at_epoch_ms: bigint}): ForecastSnapshot {
        return new ForecastSnapshot(
            obj.today_kwh,
            obj.tomorrow_kwh,
            obj.fetched_at_epoch_ms
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
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
    public static readonly BaboonSameInVersions = ["0.1.0"]
    public baboonSameInVersions() {
        return ForecastSnapshot.BaboonSameInVersions
    }
    public static binCodec(): ForecastSnapshot_UEBACodec {
        return ForecastSnapshot_UEBACodec.instance
    }
}

/** @deprecated Version 0.1.0 is deprecated, you should migrate to 0.2.0 */
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
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeF64(writer, value.today_kwh);
            BinTools.writeF64(writer, value.tomorrow_kwh);
            BinTools.writeI64(writer, value.fetched_at_epoch_ms);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): ForecastSnapshot {
        if (this !== ForecastSnapshot_UEBACodec .lazyInstance.value) {
            return ForecastSnapshot_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 0; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const today_kwh = BinTools.readF64(reader);
        const tomorrow_kwh = BinTools.readF64(reader);
        const fetched_at_epoch_ms = BinTools.readI64(reader);
        return new ForecastSnapshot(
            today_kwh,
            tomorrow_kwh,
            fetched_at_epoch_ms,
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
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