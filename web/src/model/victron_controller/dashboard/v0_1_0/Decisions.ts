// @ts-nocheck
import {BaboonGenerated, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../../BaboonSharedRuntime'
import {Decision, Decision_UEBACodec} from './Decision'

export class Decisions implements BaboonGenerated {
    private readonly _grid_setpoint: Decision | undefined;
    private readonly _input_current_limit: Decision | undefined;
    private readonly _schedule_0: Decision | undefined;
    private readonly _schedule_1: Decision | undefined;
    private readonly _zappi_mode: Decision | undefined;
    private readonly _eddi_mode: Decision | undefined;
    private readonly _weather_soc: Decision | undefined;

    constructor(grid_setpoint: Decision | undefined, input_current_limit: Decision | undefined, schedule_0: Decision | undefined, schedule_1: Decision | undefined, zappi_mode: Decision | undefined, eddi_mode: Decision | undefined, weather_soc: Decision | undefined) {
        this._grid_setpoint = grid_setpoint
        this._input_current_limit = input_current_limit
        this._schedule_0 = schedule_0
        this._schedule_1 = schedule_1
        this._zappi_mode = zappi_mode
        this._eddi_mode = eddi_mode
        this._weather_soc = weather_soc
    }

    public get grid_setpoint(): Decision | undefined {
        return this._grid_setpoint;
    }
    public get input_current_limit(): Decision | undefined {
        return this._input_current_limit;
    }
    public get schedule_0(): Decision | undefined {
        return this._schedule_0;
    }
    public get schedule_1(): Decision | undefined {
        return this._schedule_1;
    }
    public get zappi_mode(): Decision | undefined {
        return this._zappi_mode;
    }
    public get eddi_mode(): Decision | undefined {
        return this._eddi_mode;
    }
    public get weather_soc(): Decision | undefined {
        return this._weather_soc;
    }

    public toJSON(): Record<string, unknown> {
        return {
            grid_setpoint: this._grid_setpoint !== undefined ? this._grid_setpoint : undefined,
            input_current_limit: this._input_current_limit !== undefined ? this._input_current_limit : undefined,
            schedule_0: this._schedule_0 !== undefined ? this._schedule_0 : undefined,
            schedule_1: this._schedule_1 !== undefined ? this._schedule_1 : undefined,
            zappi_mode: this._zappi_mode !== undefined ? this._zappi_mode : undefined,
            eddi_mode: this._eddi_mode !== undefined ? this._eddi_mode : undefined,
            weather_soc: this._weather_soc !== undefined ? this._weather_soc : undefined
        };
    }

    public with(overrides: {grid_setpoint?: Decision | undefined; input_current_limit?: Decision | undefined; schedule_0?: Decision | undefined; schedule_1?: Decision | undefined; zappi_mode?: Decision | undefined; eddi_mode?: Decision | undefined; weather_soc?: Decision | undefined}): Decisions {
        return new Decisions(
            'grid_setpoint' in overrides ? overrides.grid_setpoint! : this._grid_setpoint,
            'input_current_limit' in overrides ? overrides.input_current_limit! : this._input_current_limit,
            'schedule_0' in overrides ? overrides.schedule_0! : this._schedule_0,
            'schedule_1' in overrides ? overrides.schedule_1! : this._schedule_1,
            'zappi_mode' in overrides ? overrides.zappi_mode! : this._zappi_mode,
            'eddi_mode' in overrides ? overrides.eddi_mode! : this._eddi_mode,
            'weather_soc' in overrides ? overrides.weather_soc! : this._weather_soc
        );
    }

    public static fromPlain(obj: {grid_setpoint: Decision | undefined; input_current_limit: Decision | undefined; schedule_0: Decision | undefined; schedule_1: Decision | undefined; zappi_mode: Decision | undefined; eddi_mode: Decision | undefined; weather_soc: Decision | undefined}): Decisions {
        return new Decisions(
            obj.grid_setpoint,
            obj.input_current_limit,
            obj.schedule_0,
            obj.schedule_1,
            obj.zappi_mode,
            obj.eddi_mode,
            obj.weather_soc
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return Decisions.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Decisions.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#Decisions'
    public baboonTypeIdentifier() {
        return Decisions.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0", "0.2.0"]
    public baboonSameInVersions() {
        return Decisions.BaboonSameInVersions
    }
    public static binCodec(): Decisions_UEBACodec {
        return Decisions_UEBACodec.instance
    }
}

/** @deprecated Version 0.1.0 is deprecated, you should migrate to 0.2.0 */
export class Decisions_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: Decisions, writer: BaboonBinWriter): unknown {
        if (this !== Decisions_UEBACodec.lazyInstance.value) {
          return Decisions_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.grid_setpoint === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                Decision_UEBACodec.instance.encode(ctx, value.grid_setpoint, buffer);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.input_current_limit === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                Decision_UEBACodec.instance.encode(ctx, value.input_current_limit, buffer);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.schedule_0 === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                Decision_UEBACodec.instance.encode(ctx, value.schedule_0, buffer);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.schedule_1 === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                Decision_UEBACodec.instance.encode(ctx, value.schedule_1, buffer);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.zappi_mode === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                Decision_UEBACodec.instance.encode(ctx, value.zappi_mode, buffer);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.eddi_mode === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                Decision_UEBACodec.instance.encode(ctx, value.eddi_mode, buffer);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.weather_soc === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                Decision_UEBACodec.instance.encode(ctx, value.weather_soc, buffer);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            if (value.grid_setpoint === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                Decision_UEBACodec.instance.encode(ctx, value.grid_setpoint, writer);
            }
            if (value.input_current_limit === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                Decision_UEBACodec.instance.encode(ctx, value.input_current_limit, writer);
            }
            if (value.schedule_0 === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                Decision_UEBACodec.instance.encode(ctx, value.schedule_0, writer);
            }
            if (value.schedule_1 === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                Decision_UEBACodec.instance.encode(ctx, value.schedule_1, writer);
            }
            if (value.zappi_mode === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                Decision_UEBACodec.instance.encode(ctx, value.zappi_mode, writer);
            }
            if (value.eddi_mode === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                Decision_UEBACodec.instance.encode(ctx, value.eddi_mode, writer);
            }
            if (value.weather_soc === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                Decision_UEBACodec.instance.encode(ctx, value.weather_soc, writer);
            }
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): Decisions {
        if (this !== Decisions_UEBACodec .lazyInstance.value) {
            return Decisions_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 7; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const grid_setpoint = (BinTools.readByte(reader) === 0 ? undefined : Decision_UEBACodec.instance.decode(ctx, reader));
        const input_current_limit = (BinTools.readByte(reader) === 0 ? undefined : Decision_UEBACodec.instance.decode(ctx, reader));
        const schedule_0 = (BinTools.readByte(reader) === 0 ? undefined : Decision_UEBACodec.instance.decode(ctx, reader));
        const schedule_1 = (BinTools.readByte(reader) === 0 ? undefined : Decision_UEBACodec.instance.decode(ctx, reader));
        const zappi_mode = (BinTools.readByte(reader) === 0 ? undefined : Decision_UEBACodec.instance.decode(ctx, reader));
        const eddi_mode = (BinTools.readByte(reader) === 0 ? undefined : Decision_UEBACodec.instance.decode(ctx, reader));
        const weather_soc = (BinTools.readByte(reader) === 0 ? undefined : Decision_UEBACodec.instance.decode(ctx, reader));
        return new Decisions(
            grid_setpoint,
            input_current_limit,
            schedule_0,
            schedule_1,
            zappi_mode,
            eddi_mode,
            weather_soc,
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return Decisions_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Decisions_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#Decisions'
    public baboonTypeIdentifier() {
        return Decisions_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new Decisions_UEBACodec())
    public static get instance(): Decisions_UEBACodec {
        return Decisions_UEBACodec.lazyInstance.value
    }
}