// @ts-nocheck
import {BaboonGeneratedLatest, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../BaboonSharedRuntime'

export class ScheduleSpec implements BaboonGeneratedLatest {
    private readonly _start_s: number;
    private readonly _duration_s: number;
    private readonly _discharge: number;
    private readonly _soc: number;
    private readonly _days: number;

    constructor(start_s: number, duration_s: number, discharge: number, soc: number, days: number) {
        this._start_s = start_s
        this._duration_s = duration_s
        this._discharge = discharge
        this._soc = soc
        this._days = days
    }

    public get start_s(): number {
        return this._start_s;
    }
    public get duration_s(): number {
        return this._duration_s;
    }
    public get discharge(): number {
        return this._discharge;
    }
    public get soc(): number {
        return this._soc;
    }
    public get days(): number {
        return this._days;
    }

    public toJSON(): Record<string, unknown> {
        return {
            start_s: this._start_s,
            duration_s: this._duration_s,
            discharge: this._discharge,
            soc: this._soc,
            days: this._days
        };
    }

    public with(overrides: {start_s?: number; duration_s?: number; discharge?: number; soc?: number; days?: number}): ScheduleSpec {
        return new ScheduleSpec(
            'start_s' in overrides ? overrides.start_s! : this._start_s,
            'duration_s' in overrides ? overrides.duration_s! : this._duration_s,
            'discharge' in overrides ? overrides.discharge! : this._discharge,
            'soc' in overrides ? overrides.soc! : this._soc,
            'days' in overrides ? overrides.days! : this._days
        );
    }

    public static fromPlain(obj: {start_s: number; duration_s: number; discharge: number; soc: number; days: number}): ScheduleSpec {
        return new ScheduleSpec(
            obj.start_s,
            obj.duration_s,
            obj.discharge,
            obj.soc,
            obj.days
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return ScheduleSpec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return ScheduleSpec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#ScheduleSpec'
    public baboonTypeIdentifier() {
        return ScheduleSpec.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0", "0.2.0", "0.3.0"]
    public baboonSameInVersions() {
        return ScheduleSpec.BaboonSameInVersions
    }
    public static binCodec(): ScheduleSpec_UEBACodec {
        return ScheduleSpec_UEBACodec.instance
    }
}

export class ScheduleSpec_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: ScheduleSpec, writer: BaboonBinWriter): unknown {
        if (this !== ScheduleSpec_UEBACodec.lazyInstance.value) {
          return ScheduleSpec_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            BinTools.writeI32(buffer, value.start_s);
            BinTools.writeI32(buffer, value.duration_s);
            BinTools.writeI32(buffer, value.discharge);
            BinTools.writeF64(buffer, value.soc);
            BinTools.writeI32(buffer, value.days);
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeI32(writer, value.start_s);
            BinTools.writeI32(writer, value.duration_s);
            BinTools.writeI32(writer, value.discharge);
            BinTools.writeF64(writer, value.soc);
            BinTools.writeI32(writer, value.days);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): ScheduleSpec {
        if (this !== ScheduleSpec_UEBACodec .lazyInstance.value) {
            return ScheduleSpec_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 0; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const start_s = BinTools.readI32(reader);
        const duration_s = BinTools.readI32(reader);
        const discharge = BinTools.readI32(reader);
        const soc = BinTools.readF64(reader);
        const days = BinTools.readI32(reader);
        return new ScheduleSpec(
            start_s,
            duration_s,
            discharge,
            soc,
            days,
        );
    }

    public static readonly BaboonDomainVersion = '0.3.0'
    public baboonDomainVersion() {
        return ScheduleSpec_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return ScheduleSpec_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#ScheduleSpec'
    public baboonTypeIdentifier() {
        return ScheduleSpec_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new ScheduleSpec_UEBACodec())
    public static get instance(): ScheduleSpec_UEBACodec {
        return ScheduleSpec_UEBACodec.lazyInstance.value
    }
}