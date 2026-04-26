// @ts-nocheck
import {BaboonGenerated, BaboonCodecContext, BaboonBinWriter, BinTools, BaboonBinReader, Lazy} from '../../../BaboonSharedRuntime'
import {CoreFactor, CoreFactor_UEBACodec} from './CoreFactor'

export class CoreState implements BaboonGenerated {
    private readonly _id: string;
    private readonly _depends_on: Array<string>;
    private readonly _last_run_outcome: string;
    private readonly _last_payload: string | undefined;
    private readonly _last_inputs: Array<CoreFactor>;
    private readonly _last_outputs: Array<CoreFactor>;

    constructor(id: string, depends_on: Array<string>, last_run_outcome: string, last_payload: string | undefined, last_inputs: Array<CoreFactor>, last_outputs: Array<CoreFactor>) {
        this._id = id
        this._depends_on = depends_on
        this._last_run_outcome = last_run_outcome
        this._last_payload = last_payload
        this._last_inputs = last_inputs
        this._last_outputs = last_outputs
    }

    public get id(): string {
        return this._id;
    }
    public get depends_on(): Array<string> {
        return this._depends_on;
    }
    public get last_run_outcome(): string {
        return this._last_run_outcome;
    }
    public get last_payload(): string | undefined {
        return this._last_payload;
    }
    public get last_inputs(): Array<CoreFactor> {
        return this._last_inputs;
    }
    public get last_outputs(): Array<CoreFactor> {
        return this._last_outputs;
    }

    public toJSON(): Record<string, unknown> {
        return {
            id: this._id,
            depends_on: this._depends_on,
            last_run_outcome: this._last_run_outcome,
            last_payload: this._last_payload !== undefined ? this._last_payload : undefined,
            last_inputs: this._last_inputs,
            last_outputs: this._last_outputs
        };
    }

    public with(overrides: {id?: string; depends_on?: Array<string>; last_run_outcome?: string; last_payload?: string | undefined; last_inputs?: Array<CoreFactor>; last_outputs?: Array<CoreFactor>}): CoreState {
        return new CoreState(
            'id' in overrides ? overrides.id! : this._id,
            'depends_on' in overrides ? overrides.depends_on! : this._depends_on,
            'last_run_outcome' in overrides ? overrides.last_run_outcome! : this._last_run_outcome,
            'last_payload' in overrides ? overrides.last_payload! : this._last_payload,
            'last_inputs' in overrides ? overrides.last_inputs! : this._last_inputs,
            'last_outputs' in overrides ? overrides.last_outputs! : this._last_outputs
        );
    }

    public static fromPlain(obj: {id: string; depends_on: Array<string>; last_run_outcome: string; last_payload: string | undefined; last_inputs: Array<CoreFactor>; last_outputs: Array<CoreFactor>}): CoreState {
        return new CoreState(
            obj.id,
            obj.depends_on,
            obj.last_run_outcome,
            obj.last_payload,
            obj.last_inputs,
            obj.last_outputs
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return CoreState.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return CoreState.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#CoreState'
    public baboonTypeIdentifier() {
        return CoreState.BaboonTypeIdentifier
    }
    public static readonly BaboonSameInVersions = ["0.2.0", "0.3.0"]
    public baboonSameInVersions() {
        return CoreState.BaboonSameInVersions
    }
    public static binCodec(): CoreState_UEBACodec {
        return CoreState_UEBACodec.instance
    }
}

/** @deprecated Version 0.2.0 is deprecated, you should migrate to 0.3.0 */
export class CoreState_UEBACodec {
    public encode(ctx: BaboonCodecContext, value: CoreState, writer: BaboonBinWriter): unknown {
        if (this !== CoreState_UEBACodec.lazyInstance.value) {
          return CoreState_UEBACodec.lazyInstance.value.encode(ctx, value, writer)
        }
    
        if (ctx === BaboonCodecContext.Indexed) {
            BinTools.writeByte(writer, 0x01);
            const buffer = new BaboonBinWriter();
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeString(buffer, value.id);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeI32(buffer, Array.from(value.depends_on).length);
            for (const item of value.depends_on) {
                BinTools.writeString(buffer, item);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeString(buffer, value.last_run_outcome);
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                if (value.last_payload === undefined) {
                BinTools.writeByte(buffer, 0);
            } else {
                BinTools.writeByte(buffer, 1);
                BinTools.writeString(buffer, value.last_payload);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeI32(buffer, Array.from(value.last_inputs).length);
            for (const item of value.last_inputs) {
                CoreFactor_UEBACodec.instance.encode(ctx, item, buffer);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            {
                const before = buffer.position();
                BinTools.writeI32(writer, before);
                BinTools.writeI32(buffer, Array.from(value.last_outputs).length);
            for (const item of value.last_outputs) {
                CoreFactor_UEBACodec.instance.encode(ctx, item, buffer);
            }
                const after = buffer.position();
                BinTools.writeI32(writer, after - before);
            }
            writer.writeAll(buffer.toBytes());
        } else {
            BinTools.writeByte(writer, 0x00)
            BinTools.writeString(writer, value.id);
            BinTools.writeI32(writer, Array.from(value.depends_on).length);
            for (const item of value.depends_on) {
                BinTools.writeString(writer, item);
            }
            BinTools.writeString(writer, value.last_run_outcome);
            if (value.last_payload === undefined) {
                BinTools.writeByte(writer, 0);
            } else {
                BinTools.writeByte(writer, 1);
                BinTools.writeString(writer, value.last_payload);
            }
            BinTools.writeI32(writer, Array.from(value.last_inputs).length);
            for (const item of value.last_inputs) {
                CoreFactor_UEBACodec.instance.encode(ctx, item, writer);
            }
            BinTools.writeI32(writer, Array.from(value.last_outputs).length);
            for (const item of value.last_outputs) {
                CoreFactor_UEBACodec.instance.encode(ctx, item, writer);
            }
        }
    }
    
    public decode(ctx: BaboonCodecContext, reader: BaboonBinReader): CoreState {
        if (this !== CoreState_UEBACodec .lazyInstance.value) {
            return CoreState_UEBACodec.lazyInstance.value.decode(ctx, reader)
        }
    
        const header = BinTools.readByte(reader);
        const useIndices = header === 0x01;
        if (useIndices) {
            for (let i = 0; i < 6; i++) {
                BinTools.readI32(reader);
                BinTools.readI32(reader);
            }
        }
        const id = BinTools.readString(reader);
        const depends_on = Array.from({ length: BinTools.readI32(reader) }, () => BinTools.readString(reader));
        const last_run_outcome = BinTools.readString(reader);
        const last_payload = (BinTools.readByte(reader) === 0 ? undefined : BinTools.readString(reader));
        const last_inputs = Array.from({ length: BinTools.readI32(reader) }, () => CoreFactor_UEBACodec.instance.decode(ctx, reader));
        const last_outputs = Array.from({ length: BinTools.readI32(reader) }, () => CoreFactor_UEBACodec.instance.decode(ctx, reader));
        return new CoreState(
            id,
            depends_on,
            last_run_outcome,
            last_payload,
            last_inputs,
            last_outputs,
        );
    }

    public static readonly BaboonDomainVersion = '0.2.0'
    public baboonDomainVersion() {
        return CoreState_UEBACodec.BaboonDomainVersion
    }
    public static readonly BaboonDomainIdentifier = 'victron_controller.dashboard'
    public baboonDomainIdentifier() {
        return CoreState_UEBACodec.BaboonDomainIdentifier
    }
    public static readonly BaboonTypeIdentifier = 'victron_controller.dashboard/:#CoreState'
    public baboonTypeIdentifier() {
        return CoreState_UEBACodec.BaboonTypeIdentifier
    }

    protected static lazyInstance = new Lazy(() => new CoreState_UEBACodec())
    public static get instance(): CoreState_UEBACodec {
        return CoreState_UEBACodec.lazyInstance.value
    }
}