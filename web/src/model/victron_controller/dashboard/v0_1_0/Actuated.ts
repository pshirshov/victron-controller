// @ts-nocheck
import {BaboonGenerated, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../../BaboonSharedRuntime'
import {ActuatedI32, ActuatedI32_UEBACodec} from './ActuatedI32'
import {ActuatedEnumName, ActuatedEnumName_UEBACodec} from './ActuatedEnumName'
import {ActuatedF64, ActuatedF64_UEBACodec} from './ActuatedF64'
import {ActuatedSchedule, ActuatedSchedule_UEBACodec} from './ActuatedSchedule'

export class Actuated implements BaboonGenerated {
    private readonly _grid_setpoint: ActuatedI32;
    private readonly _input_current_limit: ActuatedF64;
    private readonly _zappi_mode: ActuatedEnumName;
    private readonly _eddi_mode: ActuatedEnumName;
    private readonly _schedule_0: ActuatedSchedule;
    private readonly _schedule_1: ActuatedSchedule;

    constructor(grid_setpoint: ActuatedI32, input_current_limit: ActuatedF64, zappi_mode: ActuatedEnumName, eddi_mode: ActuatedEnumName, schedule_0: ActuatedSchedule, schedule_1: ActuatedSchedule) {
        this._grid_setpoint = grid_setpoint
        this._input_current_limit = input_current_limit
        this._zappi_mode = zappi_mode
        this._eddi_mode = eddi_mode
        this._schedule_0 = schedule_0
        this._schedule_1 = schedule_1
    }

    public get grid_setpoint(): ActuatedI32 {
        return this._grid_setpoint;
    }
    public get input_current_limit(): ActuatedF64 {
        return this._input_current_limit;
    }
    public get zappi_mode(): ActuatedEnumName {
        return this._zappi_mode;
    }
    public get eddi_mode(): ActuatedEnumName {
        return this._eddi_mode;
    }
    public get schedule_0(): ActuatedSchedule {
        return this._schedule_0;
    }
    public get schedule_1(): ActuatedSchedule {
        return this._schedule_1;
    }

    public toJSON(): Record<string, unknown> {
        return {
            grid_setpoint: this._grid_setpoint,
            input_current_limit: this._input_current_limit,
            zappi_mode: this._zappi_mode,
            eddi_mode: this._eddi_mode,
            schedule_0: this._schedule_0,
            schedule_1: this._schedule_1
        };
    }

    public with(overrides: {grid_setpoint?: ActuatedI32; input_current_limit?: ActuatedF64; zappi_mode?: ActuatedEnumName; eddi_mode?: ActuatedEnumName; schedule_0?: ActuatedSchedule; schedule_1?: ActuatedSchedule}): Actuated {
        return new Actuated(
            'grid_setpoint' in overrides ? overrides.grid_setpoint! : this._grid_setpoint,
            'input_current_limit' in overrides ? overrides.input_current_limit! : this._input_current_limit,
            'zappi_mode' in overrides ? overrides.zappi_mode! : this._zappi_mode,
            'eddi_mode' in overrides ? overrides.eddi_mode! : this._eddi_mode,
            'schedule_0' in overrides ? overrides.schedule_0! : this._schedule_0,
            'schedule_1' in overrides ? overrides.schedule_1! : this._schedule_1
        );
    }

    public static fromPlain(obj: {grid_setpoint: ActuatedI32; input_current_limit: ActuatedF64; zappi_mode: ActuatedEnumName; eddi_mode: ActuatedEnumName; schedule_0: ActuatedSchedule; schedule_1: ActuatedSchedule}): Actuated {
        return new Actuated(
            obj.grid_setpoint,
            obj.input_current_limit,
            obj.zappi_mode,
            obj.eddi_mode,
            obj.schedule_0,
            obj.schedule_1
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return Actuated.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Actuated.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#Actuated'
    public baboonTypeIdentifier() {
        return Actuated.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.1.0", "0.2.0", "0.3.0"]
    public baboonSameInVersions() {
        return Actuated.BaboonSameInVersions
    }
    public static binCodec(): Actuated_UEBACodec {
        return Actuated_UEBACodec.instance
    }
}

/** @deprecated Version 0.1.0 is deprecated, you should migrate to 0.3.0 */
export class Actuated_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: Actuated, writer: BaboonBinWriter): unknown {
        if (this !== Actuated_UEBACodec.lazyInstance.value) {
          return Actuated_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ActuatedI32_UEBACodec.instance.encode(ctx, value.grid_setpoint, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ActuatedF64_UEBACodec.instance.encode(ctx, value.input_current_limit, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ActuatedEnumName_UEBACodec.instance.encode(ctx, value.zappi_mode, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ActuatedEnumName_UEBACodec.instance.encode(ctx, value.eddi_mode, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ActuatedSchedule_UEBACodec.instance.encode(ctx, value.schedule_0, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                ActuatedSchedule_UEBACodec.instance.encode(ctx, value.schedule_1, buffer);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            ActuatedI32_UEBACodec.instance.encode(ctx, value.grid_setpoint, writer);
            ActuatedF64_UEBACodec.instance.encode(ctx, value.input_current_limit, writer);
            ActuatedEnumName_UEBACodec.instance.encode(ctx, value.zappi_mode, writer);
            ActuatedEnumName_UEBACodec.instance.encode(ctx, value.eddi_mode, writer);
            ActuatedSchedule_UEBACodec.instance.encode(ctx, value.schedule_0, writer);
            ActuatedSchedule_UEBACodec.instance.encode(ctx, value.schedule_1, writer);
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): Actuated {
        if (this !== Actuated_UEBACodec .lazyInstance.value) {
            return Actuated_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 6; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const grid_setpoint = ActuatedI32_UEBACodec.instance.decode(ctx, reader);
        const input_current_limit = ActuatedF64_UEBACodec.instance.decode(ctx, reader);
        const zappi_mode = ActuatedEnumName_UEBACodec.instance.decode(ctx, reader);
        const eddi_mode = ActuatedEnumName_UEBACodec.instance.decode(ctx, reader);
        const schedule_0 = ActuatedSchedule_UEBACodec.instance.decode(ctx, reader);
        const schedule_1 = ActuatedSchedule_UEBACodec.instance.decode(ctx, reader);
        return new Actuated(
            grid_setpoint,
            input_current_limit,
            zappi_mode,
            eddi_mode,
            schedule_0,
            schedule_1,
        );
    }

    public static readonly BaboonDomainVersion = '0.1.0'
    public baboonDomainVersion() {
        return Actuated_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return Actuated_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#Actuated'
    public baboonTypeIdentifier() {
        return Actuated_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new Actuated_UEBACodec())
    public static get instance(): Actuated_UEBACodec {
        return Actuated_UEBACodec.lazyInstance.value
    }
}